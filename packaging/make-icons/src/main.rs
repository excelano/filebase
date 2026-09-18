//! Turn the icon SVG into every raster the three platforms want.
//!
//! Linux ships the SVG and lets the desktop rasterize it. Neither other
//! platform has such a step: the Windows shell reads a fixed set of sizes out
//! of an icon directory, an MSIX package declares PNGs at fixed dimensions, a
//! macOS bundle carries one icon family, and every store's listing form wants
//! its own square. So every size has to exist before anything is shipped, and
//! this is the step between. It is why rasterized artefacts are committed to a
//! repository that otherwise holds only sources.
//!
//! **It makes the macOS icon too.** The fleet's older repositories render their
//! `.icns` with `sips` and `iconutil`, and both of those exist only on a Mac,
//! so none of them can produce one until it reaches that machine. `icns` is
//! pure Rust and renders here, which takes the icon off the platform session's
//! critical path. `~/notes/mac_icons_on_linux.md` measured that in odox.
//!
//! **One drawing, where the fleet's document applications have three.** The
//! rule those followed is that an application opening two kinds of file draws
//! three icons, because a file type draws its own icon by its own mechanism on
//! every platform and pointing both types at the application's drawing puts one
//! picture on both. Filebase opens a *folder*. It registers no file type, so
//! there is nothing to draw but the application.
//!
//! Everything lands in `packaging/icons/` rather than in a platform's arm.
//! There is no platform arm yet but Linux's, and a lane's build script copies
//! what it needs from here when it is written — which keeps this tool from
//! having to know which lanes exist.
//!
//!     cargo run --manifest-path packaging/make-icons/Cargo.toml
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::path::Path;

use resvg::tiny_skia;
use resvg::usvg;

// ---------------------------------------------------------------- Windows ---

/// The sizes the Windows shell asks for.
///
/// 16, 32 and 48 are the list, the desktop and the tile; 256 is what the
/// extra-large view and the file properties dialog use. 20, 24, 40, 64 and 128
/// are the same three sizes again at the display scalings Windows offers, and
/// without them the shell picks a neighbour and resamples it, which is visibly
/// softer than rendering the vector at the size wanted.
const ICO_SIZES: &[u32] = &[16, 20, 24, 32, 40, 48, 64, 128, 256];

/// Above this, an icon directory entry is stored as PNG rather than as a bitmap.
///
/// PNG entries are read by Vista and later and nothing older, and a 256-pixel
/// bitmap entry costs 256 KiB where the PNG costs a few. Below the line the
/// bitmap is kept, because it is what every consumer of an icon directory has
/// always understood and the saving there is worth nothing.
const PNG_ABOVE: u32 = 48;

// ------------------------------------------------------------------ macOS ---

/// The ten elements an `.iconset` carries, which is what `iconutil` consumes,
/// as the pixel count to render and the type that count belongs to.
///
/// **Both halves matter, and naming the type is the whole point.** Two pairs
/// render at the same pixel count under different types, because macOS
/// distinguishes a 32-pixel icon from a 16-pixel icon on a 2x display, and the
/// crate's own `add_icon` picks from the pixel size alone and so cannot tell
/// them apart. Letting it choose puts the small sizes in under legacy types.
const ICNS_ELEMENTS: &[(u32, icns::IconType)] = &[
    (16, icns::IconType::RGBA32_16x16),
    (32, icns::IconType::RGBA32_16x16_2x),
    (32, icns::IconType::RGBA32_32x32),
    (64, icns::IconType::RGBA32_32x32_2x),
    (128, icns::IconType::RGBA32_128x128),
    (256, icns::IconType::RGBA32_128x128_2x),
    (256, icns::IconType::RGBA32_256x256),
    (512, icns::IconType::RGBA32_256x256_2x),
    (512, icns::IconType::RGBA32_512x512),
    (1024, icns::IconType::RGBA32_512x512_2x),
];

// ----------------------------------------------------------- the listings ---

/// The sizes a submission form asks for.
///
/// 1024 is App Store Connect's, 1080 and 2160 are Partner Center's *Store
/// logo*, and 256 and 512 are what a support page or a press kit wants. All
/// square: no listing field on either store takes a wide one.
const LISTING_SIZES: &[u32] = &[256, 512, 1024, 1080, 2160];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let icons = here
        .parent()
        .ok_or("make-icons is not inside packaging/")?
        .join("icons");

    // Both shapes are committed sources rather than one being derived from the
    // other, because the corner is a drawing decision and a regex over somebody
    // else's markup is not where a drawing decision belongs. The square's
    // comment says which of the two is the source of record.
    let square = read(&icons.join("filebase-square.svg"))?;
    let rounded = read(&icons.join("filebase-rounded.svg"))?;

    // Every icon is the rounded drawing. A store masks what it is handed and so
    // wants the square; everything that draws what it is given — an icon
    // directory, an icon family, a launcher — wants the corner already there.
    write_ico(&rounded, &icons.join("filebase.ico"))?;
    write_icns(&rounded, &icons.join("filebase.icns"))?;

    for (shape, tree) in [("square", &square), ("rounded", &rounded)] {
        for &size in LISTING_SIZES {
            let name = format!("filebase-{shape}-{size}.png");
            write_png(tree, size, &icons.join(&name))?;
        }
    }

    println!("wrote filebase.ico, filebase.icns and {} listing squares into {}",
        LISTING_SIZES.len() * 2, icons.display());
    Ok(())
}

fn read(path: &Path) -> Result<usvg::Tree, Box<dyn std::error::Error>> {
    let source = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(usvg::Tree::from_data(&source, &usvg::Options::default())?)
}

/// The drawing on a square canvas of the given size.
///
/// Every raster here comes through this, so nothing is ever an upscale of
/// another raster: each size is the vector rendered at that size, which is the
/// difference the `.ico`'s nine entries exist for.
fn draw(tree: &usvg::Tree, size: u32) -> Result<tiny_skia::Pixmap, Box<dyn std::error::Error>> {
    let mut pixmap =
        tiny_skia::Pixmap::new(size, size).ok_or("a pixmap of that size could not be made")?;
    #[allow(clippy::cast_precision_loss)]
    let scale = size as f32 / tree.size().width();
    resvg::render(
        tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

fn write_png(tree: &usvg::Tree, size: u32, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::write(path, draw(tree, size)?.encode_png()?)?;
    Ok(())
}

fn write_ico(tree: &usvg::Tree, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for &size in ICO_SIZES {
        let pixmap = draw(tree, size)?;
        let image = ico::IconImage::from_rgba_data(size, size, pixmap.data().to_vec());
        dir.add_entry(if size > PNG_ABOVE {
            ico::IconDirEntry::encode_as_png(&image)?
        } else {
            ico::IconDirEntry::encode(&image)?
        });
    }
    dir.write(std::fs::File::create(path)?)?;
    Ok(())
}

fn write_icns(tree: &usvg::Tree, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut family = icns::IconFamily::new();
    for &(size, kind) in ICNS_ELEMENTS {
        let pixmap = draw(tree, size)?;
        let image = icns::Image::from_data(icns::PixelFormat::RGBA, size, size, pixmap.data().to_vec())?;
        // `add_icon_with_type` rather than `add_icon`: the latter picks a type
        // from the pixel count, which cannot distinguish 32x32 from 16x16@2x.
        family.add_icon_with_type(&image, kind)?;
    }
    family.write(std::io::BufWriter::new(std::fs::File::create(path)?))?;
    Ok(())
}

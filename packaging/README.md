# Packaging

What each platform needs, and what is shared. One directory per platform, and
this level for what more than one of them uses.

    icons/        the drawing, and every raster made from it
    make-icons/   the tool that makes them
    version.sh    the only thing that reads the version
    linux/        the desktop entry, the icon, and the library check
    debian/       the .deb

Linux is the only platform arm here. The Windows and macOS arms arrive with
their lanes; `make-icons` already writes what both of them will want, because
the icon is the one artefact that would otherwise sit on a platform session's
critical path.

## The version lives in one place

`Cargo.toml` holds it. `version.sh` is the only thing that reads it, and it
answers in whichever spelling a lane asks for — Debian's plain string today,
and the four-part Windows number and the two Apple ones when there is something
to ask. Nothing else parses that file.

## The icon

`icons/filebase-square.svg` is the source of record and carries the reasoning.
`icons/filebase-rounded.svg` is the same drawing with the corner already on it;
both are committed rather than one being derived, because a corner is a drawing
decision and not a regex over somebody else's markup.

Which one a place wants depends on whose job the corner is. A store masks what
it is handed and takes the square. Everything that draws what it is given — a
Windows icon directory, a macOS icon family, a Linux launcher — takes the
rounded one.

    cargo run --manifest-path packaging/make-icons/Cargo.toml

writes `filebase.ico`, `filebase.icns` and the listing squares into `icons/`.
It is pure Rust and needs no Mac: the `.icns` is built here rather than with
`sips` and `iconutil`. The rasters are committed, which is why a repository
that otherwise holds only sources has PNGs in it.

**One drawing.** The fleet's rule is that an application opening two kinds of
file draws three icons, because a file type draws its own icon by its own
mechanism and pointing both types at the application's drawing puts one picture
on both. Filebase opens a folder and registers no file type, so there is
nothing to draw but the application.

## What this package does not carry

**No media type, and no dependency on another Slipcase package.** Slipcase
Desktop and Slipcase Open open a document, so they associate themselves with
`.slpc` and depend on `slipcase-common`, which declares the type once for every
product that reads it. Filebase is handed a directory. Its desktop entry
declares `inode/directory`, which is what puts it in a file manager's *Open
With* for a folder and is the only reason the binary takes a positional
argument at all.

## Linux

    ./packaging/debian/build-deb.sh          # writes dist/*.deb
    ./packaging/linux/check-libraries.sh     # needs a display
    ./packaging/linux/install.sh             # for a build from this checkout

`debian/control.in` writes the `Depends` line by hand, and that is deliberate.
The executable links libc and libgcc and nothing else; the display stack, the
graphics driver loader and the keyboard map libraries are all opened by name at
run time, so `dpkg-shlibdeps` finds two of a dozen. `check-libraries.sh` is what
makes the hand-written list honest: it runs the binary under each display
backend, reads `/proc/PID/maps`, and reports any object that is not reachable
from a package named in `Depends`.

It needs a folder to bind to and nothing in it, because Filebase opens a window
against whatever it is pointed at. The applications that open a *document* need
a conformant fixture at this point, since a refusal takes a different path and
may not reach the graphics driver; a folder has no such distinction.

`debian/changelog` carries the version with no Debian revision — `0.1.0`, not
`0.1.0-1` — because `build-deb.sh` checks it against `Cargo.toml` and the two
have to be the same string. Its date has to name the right weekday, which
lintian checks and which has failed a sibling's every run for a release.

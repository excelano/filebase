//! Embed the Windows application manifest, and do nothing else ever.
//!
//! The rule this holds to is that nothing in this project compiles C and that a
//! build needs a Rust toolchain and nothing else. This prints two linker
//! arguments and the linker that was already linking the binary embeds
//! `packaging/windows/filebase.manifest`. No resource compiler, no object file,
//! nothing compiled that was not compiled before.
//!
//! The distinction matters because the obvious way to do this is `rc.exe` or
//! `windres`, which is also what the window icon would want — and why the icon
//! travels through `include_bytes!` instead. That rejection was of the resource
//! compiler and not of the outcome, and the linker route needs no compiler.
//!
//! **A second use for this file is a decision, not a precedent.** The one
//! opened here is narrow on purpose.
//!
//! Author: David M. Anderson
//! Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::path::Path;

fn main() {
    // The manifest lives with the rest of this platform's files rather than at
    // the crate root, which is the same rule as staying inside your own
    // directory under `packaging/`.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packaging")
        .join("windows")
        .join("filebase.manifest");

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", manifest.display());

    // Read from the environment rather than from `cfg!`, because a build script
    // is compiled for the host and `cfg!(windows)` in here answers about the
    // machine doing the building. Cross-checking from Linux with
    // `--target x86_64-pc-windows-msvc` is a thing this repository does, and it
    // would take the wrong branch.
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if os != "windows" || env != "msvc" {
        return;
    }

    // `/MANIFEST:EMBED` is MSVC's, which is why the guard above tests the
    // environment and not just the operating system: a `windows-gnu` target
    // links with something that would not understand it.
    for arg in [
        "/MANIFEST:EMBED",
        &format!("/MANIFESTINPUT:{}", manifest.display()),
    ] {
        println!("cargo:rustc-link-arg-bin=filebase={arg}");
    }
}

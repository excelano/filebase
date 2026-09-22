//! Filebase: a window over a directory of Slipcase containers.
//!
//! Everything here is what the window needs to know and none of it draws.
//! [`query`] runs a query on a thread of its own and streams rows back,
//! [`detail`] is what one container says about itself, and [`policy`] is the
//! rule that the flyleaf tree is read and not written. `src/main.rs` is the
//! drawing.
//!
//! Nothing in this crate parses a container or a query: every row comes from
//! `slipql` in `excelano/slipql` and every container fact from `slpc` in
//! `excelano/slpc-rust`. Behaviour either of them lacks goes into that library
//! rather than in here.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

// `forbid` rather than `deny`, and the difference is deliberate. The one place
// in this application that will need `unsafe` is the macOS document-open
// handler, and that lives in the binary beside the rest of the platform code —
// `src/main.rs` is `deny` for it. Reading containers and running queries can
// never acquire one, so this is shut and stays shut.
#![forbid(unsafe_code)]
#![warn(missing_docs, clippy::pedantic)]

/// The language the window draws in.
///
/// One catalogue for the whole crate, declared here rather than in the binary
/// because `detail` and `query` draw sentences of their own and a second
/// catalogue beside this one would be a second thing to keep in step. The
/// binary chooses the language and hands this the list; `po/` holds the
/// catalogues and `po/update-po.sh` keeps them level with the source.
pub mod i18n {
    pub use potext::fill;

    potext::catalog!();
}

pub mod detail;
pub mod policy;
pub mod query;

pub use detail::Detail;
pub use policy::ReadOnly;
pub use query::{Run, Update};

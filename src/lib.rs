//! Slipcase Query: a window over a directory of Slipcase containers.
//!
//! Everything here is what the window needs to know and none of it draws.
//! [`query`] runs a query on a thread of its own and streams rows back,
//! [`detail`] is what one container says about itself, and [`READ_ONLY`] is
//! the rule that the flyleaf tree is read and not written. `src/main.rs` is
//! the drawing.
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
pub mod query;

pub use detail::Detail;
pub use query::{Run, Update};

/// The rule the flyleaf tree is drawn under here: every key shown, none of
/// them editable, nothing to be added, and every string escaped the way a
/// member name is.
///
/// `flyleaf::render` is one widget serving Tommy Flyleaf, Slipcase Desktop and
/// this, and it edits. This is the application's one statement that it does
/// not write: there is no `Save` beneath the tree, and an edit that slipped
/// through would go into the in-memory document, show as though it had taken,
/// and vanish at the next selection.
///
/// Every string passes through `slpc::display_name`, because a value carrying
/// a bidirectional override reorders the line it sits on at zero width, which
/// is the spoof SPEC §3's escaping of member names exists to prevent, and a
/// flyleaf value can carry one as well as a member name can.
pub const READ_ONLY: flyleaf::ReadOnly = flyleaf::ReadOnly::displaying(slpc::display_name);

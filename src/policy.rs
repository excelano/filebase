//! The rule the flyleaf tree is drawn under here: read, never written.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::borrow::Cow;

/// Every key shown and none of them editable.
///
/// `flyleaf::render` is one widget serving Tommy Flyleaf, Slipcase Desktop and
/// this, and it edits — a `TextEdit` writes what it shows back into the document
/// the moment the field is touched. A [`Policy`](flyleaf::Policy) marking every
/// path protected is how the widget is asked for the reading half of itself,
/// and it is the only thing between this window and a document it changed.
///
/// **This is the application's one statement that it does not write.** A key
/// that slipped past `protected` would be editable in a pane with no save
/// beneath it: the edit would go into the in-memory document, show as though it
/// had taken, and vanish at the next selection. There is no path through this
/// application that writes a container, and there is no `Save` — that is
/// Slipcase Desktop's, and the day Filebase edits is the day it acquires the
/// sandbox save path and the undo stack with it.
pub struct ReadOnly;

impl flyleaf::Policy for ReadOnly {
    /// Every path, without looking at it. A tree where one key was editable and
    /// the rest were not is a harder thing to reason about than either, and
    /// nothing here wants one.
    fn protected(&self, _path: &[String]) -> bool {
        true
    }

    /// Nothing may be added either.
    ///
    /// `protected` answers for the keys that exist; this answers for the ones
    /// that do not. A tree that is not sealed draws `Add`, `add a key` and the
    /// kind picker beside every table, and those controls write into the
    /// in-memory document — an edit with no save beneath it, which is the thing
    /// `protected` exists to prevent, arriving through the one door it does not
    /// cover.
    ///
    /// Sealed at every path, for the reason every path is protected: a tree
    /// where one table accepted a new key and the rest did not is harder to
    /// reason about than either.
    fn sealed(&self, _path: &[String]) -> bool {
        true
    }

    /// How a protected string reads.
    ///
    /// SPEC §3 requires a member name be shown escaped, and `content.file` is a
    /// member name. slipcase-desktop found this by hand on Windows in 2026-08:
    /// a content file called `report<U+202E>fdp.exe` read `report\u{202E}fdp.exe` on
    /// the card and `reportfdp.exe` two rows below it in the tree, because egui
    /// gives a bidirectional formatting character zero advance width. The tree
    /// was showing the spoof the escaping exists to prevent, under a card that
    /// was not.
    ///
    /// Applied to every value rather than to `content.file` alone. `slpc::display_name`
    /// escapes what is not printable and returns the rest untouched, so an
    /// ordinary string is unchanged, and a flyleaf *value* carrying a
    /// direction override can reorder the line it sits on exactly as a member
    /// name can. Only a protected string comes through here, and here every
    /// string is protected, so this reaches all of them.
    fn display_protected<'a>(&self, value: &'a str) -> Cow<'a, str> {
        slpc::display_name(value)
    }
}

#[cfg(test)]
mod tests {
    use super::ReadOnly;
    use flyleaf::Policy as _;

    /// Would catch a path this policy forgot to protect, which is the one way
    /// a window with no save in it could offer somebody an edit.
    #[test]
    fn nothing_is_editable() {
        for path in [
            vec![],
            vec!["title".to_owned()],
            vec!["governance".to_owned(), "owner".to_owned()],
            vec!["slipcase_version".to_owned()],
        ] {
            assert!(ReadOnly.protected(&path), "{path:?} should not be editable");
        }
    }

    /// The other half of `nothing_is_editable`. `protected` answers for the
    /// keys a container has; this answers for the ones somebody could give it,
    /// and an unsealed table is the one place a window with no save in it still
    /// offers an edit.
    #[test]
    fn nothing_can_be_added() {
        for path in [
            vec![],
            vec!["governance".to_owned()],
            vec!["tags".to_owned()],
        ] {
            assert!(ReadOnly.sealed(&path), "{path:?} should accept nothing new");
        }
    }

    /// Would catch the escaping being dropped, which is the defect that reached
    /// a window once already: a right-to-left override in a value reorders the
    /// line it is drawn on and costs nothing in width to do it.
    #[test]
    fn a_direction_override_is_escaped_and_ordinary_text_is_not() {
        let spoof = "report\u{202E}fdp.exe";
        let shown = ReadOnly.display_protected(spoof);
        assert!(!shown.contains('\u{202E}'), "{shown}");

        let plain = "Master services agreement";
        assert_eq!(ReadOnly.display_protected(plain), plain);
    }
}

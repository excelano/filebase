//! What one container says about itself.
//!
//! A row in the table is what the query projected. This is everything else the
//! container holds, read when somebody selects it and not before: a scan that
//! opened every content file to fill a pane nobody is looking at would be a scan
//! that costs what an index costs, which is what `slipql` exists not to do.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::path::{Path, PathBuf};

use slpc::toml_edit::DocumentMut;

use crate::i18n::{fill, t, tn};

/// One container, opened.
pub struct Detail {
    /// Where it is, absolutely, which is what the content file is handed over from.
    pub path: PathBuf,
    /// Its path as the row named it, relative to the `from` root.
    pub relative: String,
    /// What it turned out to be.
    pub outcome: Outcome,
}

/// What opening a container produced.
///
/// Two arms, and the second is the ordinary one. `slpc` reserves `Err` for not
/// being able to read the bytes at all, which is a fact about the path rather
/// than about a container — a file removed between the scan and the click lands
/// here, and so does one somebody has no permission to read.
pub enum Outcome {
    /// Why it could not be read.
    Unreadable(String),
    /// It was read. Boxed because a flyleaf document is two orders of
    /// magnitude larger than the string beside it, and an unboxed arm would
    /// make every `Outcome` that size whichever one it holds.
    Read(Box<Contents>),
}

/// A container that was read.
pub struct Contents {
    /// The flyleaf, as the tree draws it.
    ///
    /// Mutable because `flyleaf::render` takes it that way — it is one widget
    /// serving an editor and this — and nothing here writes it back.
    /// `crate::policy::ReadOnly` is what stops the widget changing it at all.
    pub flyleaf: DocumentMut,
    /// The content file, as the card states it.
    pub content: ContentFile,
}

/// The content file, as the card states it.
pub struct ContentFile {
    /// The member `content.file` names, escaped for display.
    ///
    /// Escaped through `slpc::display_name`, which SPEC §3 requires: a content file
    /// called `report<U+202E>fdp.exe` reads as `reportfdp.exe` in a window that
    /// shows the bytes, because egui gives a bidirectional formatting character
    /// zero advance width. The name in the card is the one place a person looks
    /// to see what they are about to open.
    pub name: String,
    /// Its length uncompressed, read from the central directory.
    pub size: u64,
    /// Why this build cannot decode the content file, where it cannot.
    ///
    /// SPEC §2.5 puts encryption and compression method outside conformance, so
    /// this is a fact about the build and not a verdict on the container. Asked
    /// before anything is offered rather than read off a failure afterwards,
    /// which is the difference between a button that is not offered and a
    /// button that does not work.
    pub unreadable: Option<String>,
}

impl ContentFile {
    /// Whether this build can decode the content file.
    ///
    /// Not a promise that handing it over will succeed: the library says only
    /// that a decoder exists, and truncated bytes, a failed checksum and an i/o
    /// error are all still ahead. It is enough to decide what to offer.
    #[must_use]
    pub fn can_be_opened(&self) -> bool {
        self.unreadable.is_none()
    }

    /// The size, stated plainly.
    ///
    /// The exact count stays beside the scaled one: a card that only said
    /// "1.2 MiB" would have rounded away the number somebody opened the
    /// container to read.
    #[must_use]
    pub fn size_line(&self) -> String {
        let n = self.size;
        if n < 1024 {
            return fill(tn("{n} byte", "{n} bytes", n), &[("n", &n.to_string())]);
        }
        let units = ["KiB", "MiB", "GiB", "TiB", "PiB"];
        #[allow(clippy::cast_precision_loss)]
        let mut scaled = n as f64 / 1024.0;
        let mut unit = units[0];
        for next in &units[1..] {
            if scaled < 1024.0 {
                break;
            }
            scaled /= 1024.0;
            unit = next;
        }
        // The unit is a placeholder rather than part of the sentence: KiB and
        // MiB are the same in every language, and a translator given the whole
        // line as text would be invited to translate them.
        fill(
            t("{size} {unit} ({n} bytes)"),
            &[
                ("size", &format!("{scaled:.1}")),
                ("unit", unit),
                ("n", &n.to_string()),
            ],
        )
    }
}

impl Detail {
    /// Open a container and read what the pane shows.
    ///
    /// Returns no error of its own: every way this can go wrong is a state the
    /// pane renders rather than one the window has to handle.
    #[must_use]
    pub fn open(root: &Path, relative: &str) -> Self {
        let path = root.join(relative);
        let outcome = match slpc::Container::open(&path) {
            Ok(container) => Outcome::Read(Box::new(Contents::of(&container))),
            Err(e) => Outcome::Unreadable(e.to_string()),
        };
        Self {
            path,
            relative: relative.to_owned(),
            outcome,
        }
    }

    /// The content file, where there is one to hand over.
    #[must_use]
    pub fn content(&self) -> Option<&ContentFile> {
        match &self.outcome {
            Outcome::Unreadable(_) => None,
            Outcome::Read(contents) => Some(&contents.content),
        }
    }

    /// Put the content file in a directory, under its own name, and answer where.
    ///
    /// The directory is the caller's to make and to remove. Nothing is ever
    /// written beside the container: a window that extracted into the directory
    /// it was reading would be changing what it was asked to look at.
    ///
    /// # Errors
    ///
    /// When the container cannot be opened or its content file cannot be decoded,
    /// when the content file's name will not resolve to a path inside `dir`, and
    /// when the write fails. A container that was readable when the row was
    /// scanned can be any of these by the time somebody clicks it.
    pub fn extract_to(&self, dir: &Path) -> slpc::Result<PathBuf> {
        let mut container = slpc::Container::open(&self.path)?;
        // `content_path` is what keeps a member named `../x` or `C:\x` from
        // deciding where this writes. The name is the container's and is not
        // this application's to trust.
        let out = slpc::content_path(dir, container.content_name())?;
        let mut reader = container.content()?;
        let mut file = std::fs::File::create(&out)?;
        std::io::copy(&mut reader, &mut file)?;
        Ok(out)
    }
}

impl Contents {
    fn of<R: std::io::Read + std::io::Seek>(container: &slpc::Container<R>) -> Self {
        Self {
            flyleaf: container.flyleaf().clone(),
            content: ContentFile {
                name: slpc::display_name(container.content_name()).into_owned(),
                // A central directory that will not say is not a reason to
                // refuse the container: the card says zero and the rest of the
                // pane is still worth reading.
                size: container.content_size().unwrap_or(0),
                unreadable: container
                    .check_content_readable()
                    .err()
                    .map(|why| why.to_string()),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ContentFile;

    fn content_file(size: u64) -> ContentFile {
        ContentFile {
            name: "a.pdf".to_owned(),
            size,
            unreadable: None,
        }
    }

    /// Would catch the exact byte count being dropped once a content file is large
    /// enough to scale, which is the number somebody opened the container to
    /// read and the one a rounded line cannot give back.
    #[test]
    fn a_scaled_size_still_states_the_bytes() {
        let line = content_file(1_536).size_line();
        assert!(line.contains("1.5 KiB"), "{line}");
        assert!(line.contains("1536"), "{line}");
    }

    /// Would catch a content file under a kibibyte being scaled anyway, which would
    /// read "0.9 KiB (900 bytes)" where the bytes alone are the whole answer.
    #[test]
    fn a_small_content_file_is_stated_in_bytes_alone() {
        assert_eq!(content_file(900).size_line(), "900 bytes");
        assert_eq!(content_file(1).size_line(), "1 byte");
        // Conformant under SPEC §2.3, and the card says nothing more about it.
        assert_eq!(content_file(0).size_line(), "0 bytes");
    }

    /// Would catch the scaling stopping at one unit, which would report a
    /// gibibyte content file as four figures of mebibytes.
    #[test]
    fn scaling_climbs_past_the_first_unit() {
        assert!(content_file(5 * 1024 * 1024).size_line().contains("5.0 MiB"));
        assert!(content_file(3 * 1024 * 1024 * 1024)
            .size_line()
            .contains("3.0 GiB"));
    }

    /// Would catch a content file this build cannot decode being offered anyway,
    /// which is a button that does not work rather than a button that is not
    /// there.
    #[test]
    fn a_content_file_that_cannot_be_decoded_is_not_offered() {
        assert!(content_file(10).can_be_opened());
        let encrypted = ContentFile {
            unreadable: Some("encrypted".to_owned()),
            ..content_file(10)
        };
        assert!(!encrypted.can_be_opened());
    }
}

//! Running a query without stopping the window.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use slipql::ast::Query;
use slipql::{Row, Tally};

use crate::i18n::{fill, t};

/// The most rows one run keeps.
///
/// A scan is one click away from a directory nobody meant to ask about — a home
/// directory, a mounted share — and the window holds its rows where the command
/// streams them to a pipe and forgets them. So there is a ceiling, and reaching
/// it stops the scan rather than filling memory with an answer nobody asked
/// for. The status line says when it was reached, because a truncated answer
/// that does not say so is a wrong answer.
pub const MOST_ROWS: usize = 5_000;

/// How many rows travel in one message.
///
/// Rows arrive one at a time from the scan and cross the channel in batches,
/// because a repaint per row is a repaint per file opened and a large scan
/// would spend the window's frame budget on rows nobody has scrolled to yet.
const BATCH: usize = 64;

/// How long a partial batch waits before the window is shown it.
///
/// Only checked when a row arrives, which is the only moment there is anything
/// to flush. It is what makes a slow scan over a few big containers show its
/// first row when it finds it rather than when it finishes.
const FLUSH_AFTER: Duration = Duration::from_millis(100);

/// What a run sends back.
enum Message {
    /// Rows, in scan order.
    Rows(Vec<Row>),
    /// The scan is over, and this is what it noticed on the way.
    Done(Notices),
}

/// What a finished run has to say besides its rows.
///
/// Sentences rather than the `Tally` they came from, because a tally's
/// mismatches borrow it and the tally stays on the worker thread. Formatting
/// them there also keeps the wording in one place: these are the command's own
/// notices, minus its `note:` prefix, which is a pipe's convention and not a
/// window's.
#[derive(Debug, Default, Clone)]
pub struct Notices {
    /// One line per file the scan could not use.
    pub skipped: Vec<String>,
    /// One line per comparison that crossed type classes.
    pub mismatches: Vec<String>,
    /// Whether the scan stopped at [`MOST_ROWS`] rather than at the end.
    pub truncated: bool,
}

impl Notices {
    /// Whether there is anything to report.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skipped.is_empty() && self.mismatches.is_empty() && !self.truncated
    }

    /// How many lines there are to read.
    ///
    /// Not the inverse of [`is_empty`](Self::is_empty) and deliberately not
    /// called `len`: a truncated run has nothing to read and something to say,
    /// so the two answer different questions and a pair that shared a name
    /// would invite one to be read as the other.
    #[must_use]
    pub fn lines(&self) -> usize {
        self.skipped.len() + self.mismatches.len()
    }
}

/// A query that is running, or has finished.
///
/// Held by the window across frames. [`Run::drain`] is called once a frame and
/// returns what changed, so the window never blocks on a scan and a scan never
/// touches a widget.
pub struct Run {
    /// The rows so far, in scan order.
    rows: Vec<Row>,
    /// The column names, where the query fixes them. `select *` does not: its
    /// columns are the union over the rows, so they are recomputed as rows
    /// arrive.
    fixed_columns: Option<Vec<String>>,
    /// The columns as they stand, which is the fixed set or the union so far.
    columns: Vec<String>,
    /// What the worker sends, until it is done and this is `None`.
    inbox: Option<Receiver<Message>>,
    /// What the scan noticed, once it is over.
    notices: Option<Notices>,
}

/// What one frame's [`drain`](Run::drain) changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Update {
    /// Nothing arrived. The window draws what it drew last frame.
    Nothing,
    /// Rows arrived, or the column set grew.
    Rows,
    /// The scan finished this frame.
    Finished,
}

impl Run {
    /// Start a query, with the scan on a thread of its own.
    ///
    /// Returns at once. Nothing has been read from disk when this comes back —
    /// `slipql::execute` opens the `from` directory and yields nothing further
    /// until the iterator is asked, which is what makes putting the iterator on
    /// a thread the whole of the concurrency here.
    ///
    /// # Errors
    ///
    /// When the `from` root cannot be scanned at all, and when the scanning
    /// thread cannot be spawned. A file inside the root that cannot be read is
    /// a notice rather than an error, and the query still answers.
    pub fn start(query: &Query, repaint: impl Fn() + Send + 'static) -> slipql::Result<Self> {
        let mut results = slipql::execute(query)?;
        let fixed_columns = results.columns();
        let (sender, inbox) = mpsc::channel();

        // Detached and never joined. It ends when the scan ends, or when the
        // window drops the receiver — the send fails, the iterator drops, and
        // dropping the iterator is how slipql cancels a scan. That is also what
        // makes starting a second query cancel the first: the window replaces
        // this `Run`, the old receiver goes with it, and the old thread stops
        // at its next row rather than reading the rest of the tree for an
        // answer nobody will see.
        std::thread::Builder::new()
            .name("filebase-query".to_owned())
            .spawn(move || {
                let mut batch = Vec::with_capacity(BATCH);
                let mut last_flush = Instant::now();
                let mut sent = 0usize;
                let mut truncated = false;

                for row in results.by_ref() {
                    batch.push(row);
                    if batch.len() >= BATCH || last_flush.elapsed() >= FLUSH_AFTER {
                        sent += batch.len();
                        if sender.send(Message::Rows(batch)).is_err() {
                            return;
                        }
                        batch = Vec::with_capacity(BATCH);
                        last_flush = Instant::now();
                        repaint();
                    }
                    // Against everything yielded, sent and waiting both, so the
                    // ceiling is the number of rows the window will hold rather
                    // than the size of the last batch — which is what it would
                    // measure if the batch alone were counted, and that batch is
                    // emptied every flush.
                    if over_ceiling(sent + batch.len()) {
                        truncated = true;
                        break;
                    }
                }

                if !batch.is_empty() && sender.send(Message::Rows(batch)).is_err() {
                    return;
                }
                // The tally is complete only once the iterator is, which for a
                // truncated run means complete as far as the scan went. Taking
                // it consumes the iterator, which is also what cancels the rest
                // of the scan on the truncated path.
                let notices = notices_of(&results.into_tally(), truncated);
                let _ = sender.send(Message::Done(notices));
                repaint();
            })
            .map_err(|source| slipql::Error::Source {
                root: query.from.root.clone(),
                source,
            })?;

        Ok(Self {
            rows: Vec::new(),
            columns: fixed_columns.clone().unwrap_or_default(),
            fixed_columns,
            inbox: Some(inbox),
            notices: None,
        })
    }

    /// Take whatever the scan has sent since the last frame.
    ///
    /// Called once a frame and never blocks. The loop is bounded by what is in
    /// the channel rather than by a count: a batch is already sized so that
    /// draining every one of them costs a frame nothing, and stopping early
    /// would leave rows sitting in the channel behind a window that had stopped
    /// asking for repaints.
    pub fn drain(&mut self) -> Update {
        let Some(inbox) = &self.inbox else {
            return Update::Nothing;
        };
        let mut update = Update::Nothing;
        loop {
            match inbox.try_recv() {
                Ok(Message::Rows(rows)) => {
                    self.rows.extend(rows);
                    update = Update::Rows;
                }
                Ok(Message::Done(notices)) => {
                    self.notices = Some(notices);
                    self.inbox = None;
                    update = Update::Finished;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                // The worker ended without saying so, which means it panicked:
                // the channel closes when its sender drops. Nothing is left to
                // wait for, and the rows already in hand are still rows, so the
                // run is finished with whatever it noticed being nothing.
                Err(TryRecvError::Disconnected) => {
                    self.notices.get_or_insert_with(Notices::default);
                    self.inbox = None;
                    update = Update::Finished;
                    break;
                }
            }
        }
        if update != Update::Nothing && self.fixed_columns.is_none() {
            self.columns = slipql::render::column_union(&self.rows);
        }
        update
    }

    /// Whether the scan is still going.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.inbox.is_some()
    }

    /// The rows so far, in scan order.
    #[must_use]
    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    /// The columns to draw, which for `select *` grows as rows arrive.
    #[must_use]
    pub fn columns(&self) -> &[String] {
        &self.columns
    }

    /// What the scan noticed, once it is over.
    #[must_use]
    pub fn notices(&self) -> Option<&Notices> {
        self.notices.as_ref()
    }

    /// The count under the table: how many rows, and what else there is to
    /// know about them.
    #[must_use]
    pub fn status(&self) -> String {
        let n = self.rows.len();
        let rows = fill(
            crate::i18n::tn("{n} row", "{n} rows", n as u64),
            &[("n", &n.to_string())],
        );
        if self.is_running() {
            return fill(t("{rows} so far…"), &[("rows", &rows)]);
        }
        let Some(notices) = &self.notices else {
            return rows;
        };
        let mut parts = vec![rows];
        if notices.truncated {
            parts.push(fill(
                t("stopped at {most}; narrow the query or add a limit"),
                &[("most", &MOST_ROWS.to_string())],
            ));
        }
        if !notices.skipped.is_empty() {
            let n = notices.skipped.len();
            parts.push(fill(
                crate::i18n::tn("{n} skipped", "{n} skipped", n as u64),
                &[("n", &n.to_string())],
            ));
        }
        if !notices.mismatches.is_empty() {
            let n = notices.mismatches.len();
            parts.push(fill(
                crate::i18n::tn(
                    "{n} comparison across types",
                    "{n} comparisons across types",
                    n as u64,
                ),
                &[("n", &n.to_string())],
            ));
        }
        parts.join(" · ")
    }
}

/// Whether this many rows have reached the ceiling.
///
/// Split out so the comparison is in one place and the test below can reach it
/// without a scan. `>=` and not `==`, because a batch flushes in whole
/// batches and the count steps past the ceiling rather than landing on it.
fn over_ceiling(rows: usize) -> bool {
    rows >= MOST_ROWS
}

/// The tally's sentences, which are the command's minus its `note:` prefix.
fn notices_of(tally: &Tally, truncated: bool) -> Notices {
    Notices {
        skipped: tally
            .skipped()
            .iter()
            .map(|skipped| {
                fill(
                    t("skipped {path}: {reason}"),
                    &[
                        ("path", &skipped.path.display().to_string()),
                        ("reason", &skipped.reason),
                    ],
                )
            })
            .collect(),
        mismatches: tally
            .mismatches()
            .into_iter()
            .map(|(path, found, against, count)| {
                let rows = fill(
                    crate::i18n::tn("{n} row", "{n} rows", count as u64),
                    &[("n", &count.to_string())],
                );
                fill(
                    t("{column}: {found} in {rows}, {against} in the query; not matched"),
                    &[
                        ("column", &path.to_string()),
                        ("found", &found.to_string()),
                        ("rows", &rows),
                        ("against", &against.to_string()),
                    ],
                )
            })
            .collect(),
        truncated,
    }
}

/// One cell, as the table draws it.
///
/// A column a row does not have is blank rather than absent, because flyleaf
/// keys are ad hoc by design and a query that projects one is asking whether it
/// is there. `slipql::Value`'s own `Display` writes a string as itself and
/// everything else as TOML, which is what a person who authored the flyleaf
/// would recognise.
#[must_use]
pub fn cell(row: &Row, column: &str) -> String {
    row.cells
        .iter()
        .find(|cell| cell.column == column)
        .and_then(|cell| cell.value.as_ref())
        .map_or_else(String::new, ToString::to_string)
}

#[cfg(test)]
mod tests {
    use super::{cell, over_ceiling, Notices, MOST_ROWS};
    use slipql::{Cell, Row};

    fn row(cells: &[(&str, Option<&str>)]) -> Row {
        Row {
            path: "a.slpc".to_owned(),
            cells: cells
                .iter()
                .map(|(column, value)| Cell {
                    column: (*column).to_owned(),
                    value: value.map(|v| slipql::Value::String(v.to_owned())),
                })
                .collect(),
        }
    }

    /// Would catch the ceiling being compared with `==`, which a count
    /// overshooting by one row would step straight past — leaving a scan of a
    /// home directory running to its end with nothing stopping it.
    #[test]
    fn the_ceiling_stops_a_count_that_overshoots_it() {
        assert!(over_ceiling(MOST_ROWS + 5));
        assert!(over_ceiling(MOST_ROWS));
        assert!(!over_ceiling(MOST_ROWS - 1));
    }

    /// Would catch a missing key rendering as the word `None`, which is what
    /// `Option`'s own `Display` would have put in the cell.
    #[test]
    fn a_column_the_row_does_not_have_is_blank() {
        let r = row(&[("title", Some("MSA")), ("owner", None)]);
        assert_eq!(cell(&r, "title"), "MSA");
        assert_eq!(cell(&r, "owner"), "");
        assert_eq!(cell(&r, "never projected"), "");
    }

    /// Would catch a truncated run reporting nothing to say, which is how a
    /// wrong answer would reach somebody without a word about it: the rows are
    /// real and the set is not the one they asked for.
    #[test]
    fn a_truncated_run_is_not_an_empty_set_of_notices() {
        let quiet = Notices::default();
        assert!(quiet.is_empty());
        let cut = Notices {
            truncated: true,
            ..Notices::default()
        };
        assert!(!cut.is_empty());
        // Nothing to *read*, and still something to say. The two are different
        // questions, which is why only one of them is called `is_empty`.
        assert_eq!(cut.lines(), 0);
    }
}

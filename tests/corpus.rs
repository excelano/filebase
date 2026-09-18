//! The window's engine over a real directory of real containers.
//!
//! The unit tests beside each module test one decision each. This tests the
//! thing the window actually does: pack containers, bind a folder, run a query
//! through the worker thread, drain it the way a frame does, and open what a
//! click would open. It is the nearest thing to a walkthrough that runs without
//! a display, and it was written because the display this was built on cannot
//! be captured — a Mutter XWayland window is not readable through X11.
//!
//! Fixtures are packed here rather than committed, the same rule slipql's tests
//! follow: a binary in the repository is a fixture nobody can read a diff of.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use filebase::detail::Outcome;
use filebase::query::{Run, Update};
use filebase::Detail;

/// Pack one container under `dir`, with `metadata` as its TOML.
fn pack(dir: &Path, name: &str, metadata: &str) {
    std::fs::create_dir_all(dir).unwrap();
    let payload = dir.join(name);
    std::fs::write(&payload, format!("the bytes of {name}")).unwrap();
    // `[payload]` goes last, because a TOML table header takes everything after
    // it: with the header first, `title` and `status` land as `payload.title`
    // and `payload.status`, every query against them finds nothing, and the
    // rows come back empty rather than wrong-looking. Cost an hour the first
    // time these fixtures were written.
    let doc: slpc::toml_edit::DocumentMut = format!(
        "slipcase_version = \"{}\"\n{metadata}\n[payload]\nfile = \"{name}\"\n",
        slpc::VERSION
    )
    .parse()
    .unwrap();
    let out = std::fs::File::create(dir.join(format!("{name}.slpc"))).unwrap();
    slpc::pack_file(&payload, doc, out).unwrap();
    std::fs::remove_file(&payload).unwrap();
}

/// A tree with something of every shape the window has to survive.
fn corpus() -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    let dir = root.path();

    pack(
        dir,
        "msa.pdf",
        r#"title = "Master services agreement"
status = "draft"
pages = 42
tags = ["legal", "draft"]
[governance]
owner = "Kim"
"#,
    );
    pack(
        &dir.join("2026"),
        "renewal.docx",
        r#"title = "Renewal, 2026"
status = "draft"
pages = 6
[governance]
owner = "Lee"
"#,
    );
    pack(
        &dir.join("2026").join("q3"),
        "q3.xlsx",
        r#"title = "Q3 report"
status = "signed"
pages = 12
"#,
    );
    // A container whose `pages` is a string where the others have an integer.
    // Ad-hoc keys are normal in metadata, so this is not a malformed file — it
    // is the case the mismatch tally exists for.
    pack(
        &dir.join("2026").join("q3"),
        "notes.txt",
        r#"title = "Field notes"
status = "draft"
pages = "many"
"#,
    );

    // Named like a container and not one. The scan takes `.slpc` by extension
    // alone and never sniffs, so this is reached, refused and reported.
    std::fs::write(dir.join("2026").join("broken.slpc"), b"not a container").unwrap();
    // Not named like one, so the scan never looks at it at all.
    std::fs::write(dir.join("2026").join("README.md"), b"plain").unwrap();

    root
}

/// Run a query to the end the way a sequence of frames would, and answer the
/// finished run.
///
/// The drain loop is what `eframe::App::ui` does once a frame, minus the
/// drawing. `Update::Finished` is what ends it, which is also what proves the
/// worker sends it: a run that never said so would hang here rather than pass.
fn run_to_end(folder: &Path, recursive: bool, text: &str) -> Run {
    let source = slipql::ast::Source {
        root: folder.to_path_buf(),
        recursive,
    };
    let query = slipql::parse_with(text, Some(&source)).unwrap();
    let mut run = Run::start(&query, || {}).unwrap();
    loop {
        if run.drain() == Update::Finished {
            return run;
        }
        std::thread::yield_now();
    }
}

fn cells(run: &Run, column: &str) -> Vec<String> {
    run.rows()
        .iter()
        .map(|row| filebase::query::cell(row, column))
        .collect()
}

/// Would catch the window binding a folder and scanning the whole tree under
/// it: `recursive` is opt-in, and a file manager that descended by default
/// would answer a question nobody asked and cost the time of doing it.
#[test]
fn a_bound_folder_does_not_descend_unless_asked() {
    let root = corpus();

    let shallow = run_to_end(root.path(), false, "select @path, title");
    assert_eq!(cells(&shallow, "@path"), vec!["msa.pdf.slpc"]);

    let deep = run_to_end(root.path(), true, "select @path, title");
    let mut paths = cells(&deep, "@path");
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "2026/q3/notes.txt.slpc",
            "2026/q3/q3.xlsx.slpc",
            "2026/renewal.docx.slpc",
            "msa.pdf.slpc",
        ]
    );
}

/// Would catch a `where` clause being dropped between the box and the scan,
/// which is the defect that would look most like working: rows appear, they are
/// simply the wrong rows.
#[test]
fn a_filter_reaches_the_scan() {
    let root = corpus();
    let run = run_to_end(
        root.path(),
        true,
        r#"select @path, title where status = "draft""#,
    );
    let mut titles = cells(&run, "title");
    titles.sort();
    assert_eq!(
        titles,
        vec!["Field notes", "Master services agreement", "Renewal, 2026"]
    );
    // A row that has no such key renders blank rather than dropping the row or
    // failing the query, which is what makes ad-hoc metadata queryable at all:
    // only two of these three containers carry `governance.owner`.
    let run = run_to_end(
        root.path(),
        true,
        r#"select title, governance.owner where status = "draft""#,
    );
    let owners = cells(&run, "governance.owner");
    assert_eq!(owners.len(), 3);
    assert_eq!(owners.iter().filter(|o| o.is_empty()).count(), 1, "{owners:?}");
}

/// Would catch `select *`'s columns being taken from the first row, which is
/// the obvious wrong implementation and looks right until a row carries a key
/// the first one did not.
#[test]
fn select_star_takes_the_union_over_the_rows() {
    let root = corpus();
    let run = run_to_end(root.path(), true, "select *");
    let columns = run.columns();
    assert!(columns.contains(&"@path".to_owned()), "{columns:?}");
    // `governance.owner` is on two of the four containers and `tags` on one.
    assert!(
        columns.contains(&"governance.owner".to_owned()),
        "{columns:?}"
    );
    assert!(columns.contains(&"tags".to_owned()), "{columns:?}");
}

/// Would catch a file the scan could not use being swallowed, which is the one
/// failure mode that turns a short answer into a wrong answer with nothing on
/// screen to say so.
#[test]
fn what_the_scan_could_not_use_is_reported() {
    let root = corpus();
    let run = run_to_end(root.path(), true, "select @path");
    let notices = run.notices().expect("a finished run has notices");

    assert_eq!(notices.skipped.len(), 1, "{:?}", notices.skipped);
    assert!(
        notices.skipped[0].contains("broken.slpc"),
        "{:?}",
        notices.skipped
    );
    // `README.md` is not named like a container, so it is not skipped — it was
    // never a candidate. A scan that reported it would report every file on the
    // disk.
    assert!(
        !notices.skipped.iter().any(|line| line.contains("README")),
        "{:?}",
        notices.skipped
    );
    assert!(!notices.truncated);
    assert!(run.status().contains("1 skipped"), "{}", run.status());
}

/// Would catch a comparison across types being silently unknown, which is what
/// hides a metadata authoring mistake: `pages = "many"` against `pages > 10` is
/// neither true nor false, and the row's absence is the only evidence.
#[test]
fn a_comparison_across_types_is_counted_and_said() {
    let root = corpus();
    let run = run_to_end(root.path(), true, "select @path, title where pages > 10");

    let mut titles = cells(&run, "title");
    titles.sort();
    assert_eq!(titles, vec!["Master services agreement", "Q3 report"]);

    let notices = run.notices().unwrap();
    assert_eq!(notices.mismatches.len(), 1, "{:?}", notices.mismatches);
    let line = &notices.mismatches[0];
    assert!(line.contains("pages"), "{line}");
    assert!(line.contains("string"), "{line}");
    assert!(line.contains("integer"), "{line}");
    assert!(
        run.status().contains("1 comparison across types"),
        "{}",
        run.status()
    );
}

/// Would catch the detail pane being opened against the wrong path: rows carry
/// a path relative to the `from` root, and joining it to anything else finds
/// either nothing or, worse, a different container of the same name in a
/// sibling directory.
#[test]
fn a_selected_row_opens_the_container_the_row_names() {
    let root = corpus();
    let run = run_to_end(root.path(), true, r#"select @path where title = "Q3 report""#);
    assert_eq!(run.rows().len(), 1);

    let relative = run.rows()[0].path.clone();
    assert_eq!(relative, "2026/q3/q3.xlsx.slpc");

    let detail = Detail::open(root.path(), &relative);
    let Outcome::Read(contents) = &detail.outcome else {
        panic!("the container the scan just read should open");
    };
    assert_eq!(contents.payload.name, "q3.xlsx");
    assert!(contents.payload.can_be_opened());
    assert_eq!(
        contents.metadata["title"].as_str(),
        Some("Q3 report"),
        "the pane shows the container's own metadata"
    );
}

/// Would catch a container that cannot be read taking the window down, or
/// taking the pane blank: a file removed or replaced between the scan and the
/// click is ordinary and has to render as a sentence.
#[test]
fn a_container_that_will_not_open_is_a_sentence_and_not_a_panic() {
    let root = corpus();
    let detail = Detail::open(root.path(), "2026/broken.slpc");
    match &detail.outcome {
        Outcome::Unreadable(why) => assert!(!why.is_empty()),
        Outcome::Read(_) => panic!("fifteen bytes of text are not a container"),
    }
    assert!(detail.payload().is_none());

    let gone = Detail::open(root.path(), "nothing/here.slpc");
    assert!(matches!(gone.outcome, Outcome::Unreadable(_)));
}

/// Would catch the payload being written beside the container it came from,
/// which would change the directory the window was asked to look at — and would
/// do it on a directory somebody may have no business writing to.
#[test]
fn a_payload_is_handed_over_from_somewhere_else() {
    let root = corpus();
    let scratch = tempfile::tempdir().unwrap();

    let detail = Detail::open(root.path(), "msa.pdf.slpc");
    let out = detail.extract_to(scratch.path()).unwrap();

    assert_eq!(out.parent(), Some(scratch.path()));
    assert_eq!(out.file_name().unwrap(), "msa.pdf");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "the bytes of msa.pdf"
    );

    let beside: Vec<PathBuf> = std::fs::read_dir(root.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(
        !beside.iter().any(|p| p.file_name().unwrap() == "msa.pdf"),
        "nothing was written beside the container: {beside:?}"
    );
}

/// Would catch `limit` being read by the window rather than by the scan, which
/// would be a scan that reads the whole tree and throws most of it away.
#[test]
fn a_limit_stops_the_scan() {
    let root = corpus();
    let run = run_to_end(root.path(), true, "select @path limit 2");
    assert_eq!(run.rows().len(), 2);
    assert!(!run.notices().unwrap().truncated, "a limit is not a ceiling");
    assert!(run.status().starts_with("2 rows"), "{}", run.status());
}

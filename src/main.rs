//! Filebase: a query over a directory of Slipcase containers, and what came back.
//
// Author: David M. Anderson
// Built with AI assistance (Claude, Anthropic)

// `deny` rather than `forbid`, and the difference is the whole of the
// exception. `forbid` cannot be lifted anywhere beneath it, and receiving a
// document from macOS needs one Objective-C method that cannot be written
// without `unsafe`. That module is not here yet — it arrives with the Mac lane,
// as `opened_document.rs`, and it is named here so that the day it is needed is
// not the day this attribute gets relitigated. `src/lib.rs`, which is where
// containers are read, keeps `forbid` untouched.
#![deny(unsafe_code)]
#![warn(clippy::pedantic)]
// Windows creates a console for a console-subsystem process, and a file manager
// launching this one is not attached to a terminal, so a double-clicked
// container would open a black console window behind the application. The
// attribute is ignored everywhere else, and it is off in a debug build because
// that is where a panic message still has somewhere to go.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Which way the desktop's light and dark setting points. A module rather than a
// few lines here because only one of the three platforms needs any of it, and
// the one value that is a judgement rather than a reading wants somewhere to be
// argued and tested.
mod system_theme;

use std::path::PathBuf;
use std::sync::mpsc;

use eframe::egui;

use filebase::detail::Outcome;
use filebase::i18n::{fill, t};
use filebase::query::{self, Run, Update};
use filebase::{Detail, ReadOnly};

/// Every language this application is translated into.
///
/// German alone, and the tree inside the window is translated by `flyleaf`'s
/// own catalogue rather than by this one — `set_language` below hands the tag
/// to both.
const CATALOGUES: &[(&str, &str)] = &[
    ("de", include_str!("../po/de.po")),
    // Debug builds alone, so a release carries nothing of it. `po/pseudo.sh`
    // says what it finds: a string still in English never went through `t`,
    // and a label with its end cut off was laid out to the width of English.
    #[cfg(debug_assertions)]
    ("en-x-pseudo", include_str!("../po/en-x-pseudo.po")),
];

/// The window's identity to the desktop environment.
///
/// A Wayland compositor matches this against the basename of the `.desktop`
/// entry to find the window's icon and its name, so the two have to agree. The
/// packaging installs that entry; this is the half that lives in the binary.
const APP_ID: &str = "filebase";

/// The window's icon on Windows, which has no `.desktop` entry to find one in.
///
/// `APP_ID` above is how Linux answers this question and it does nothing here:
/// `with_app_id` is Wayland's `xdg_toplevel.set_app_id` and neither egui,
/// eframe nor winit turns it into anything on Windows.
#[cfg(target_os = "windows")]
const WINDOW_ICON: &[u8] = include_bytes!("../packaging/windows/filebase.ico");

/// The icon at the largest size the drawing carries without being upscaled.
///
/// A window gets one image and Windows scales it to 16 in the title bar and 32
/// in the task bar, doubling both at 200%. 64 is a whole multiple of those four,
/// so each is an integer downsample of the same drawing. It is not a multiple of
/// what the intermediate scalings ask for — 125% wants 20 and 40, 150% wants 24
/// and 48 — and those are resampled rather than downsampled evenly. 64 stays the
/// choice because it is the largest entry no scaling has to enlarge.
///
/// A failure here is not worth a message: the window gets eframe's default icon
/// and everything else about the application works.
#[cfg(target_os = "windows")]
fn window_icon() -> Option<egui::IconData> {
    let directory = ico::IconDir::read(std::io::Cursor::new(WINDOW_ICON)).ok()?;
    let entry = directory.entries().iter().find(|e| e.width() == 64)?;
    let image = entry.decode().ok()?;
    Some(egui::IconData {
        rgba: image.rgba_data().to_vec(),
        width: image.width(),
        height: image.height(),
    })
}

/// The query a new window starts with.
///
/// `select *` rather than an empty box: a person who has just chosen a folder
/// wants to see what is in it, and the columns of `select *` are the union of
/// what the containers actually say — which is the fastest way to learn what
/// there is to query. It is also a working example of the language sitting in
/// the box where a first query gets typed.
const FIRST_QUERY: &str = "select *";

fn main() -> eframe::Result {
    // One reading of the platform, handed to this application and to the widget
    // that draws the metadata tree inside its window. `flyleaf` carries its own
    // catalogue — a published crate has to — and takes a tag rather than a
    // catalogue, so a version skew between the two costs nothing. Asked once
    // and passed on rather than asked twice, because a tree in a different
    // language from the window around it would be worse than an English one.
    //
    // Both answers are discarded: they say which catalogue matched, and a
    // language with no catalogue is a window in English, which is the fallback
    // either way.
    if let Some(language) = potext::preferred() {
        let _ = filebase::i18n::set_language(&language, CATALOGUES);
        let _ = flyleaf::set_language(&language);
    }

    // One positional directory, which is what a file manager hands an
    // application it was asked to open a folder with, and what a shell gives
    // when somebody types the name of the folder they are standing in. Nothing
    // here is a command-line interface — `slipql` is that, and it is the same
    // engine — so there are no flags, and a path that is not a directory is
    // declined by the window rather than by an argument parser.
    //
    // macOS is the one platform that does not deliver this as `argv[1]`: it
    // sends an Apple Event, which `opened_document.rs` will answer when the Mac
    // lane is built. Until then this arm is Linux and Windows.
    let opened = std::env::args_os().nth(1).map(PathBuf::from);

    let viewport = egui::ViewportBuilder::default()
        .with_app_id(APP_ID)
        .with_inner_size([1040.0, 680.0])
        .with_min_inner_size([680.0, 420.0]);

    // **Saying nothing here is not neutral on macOS, and it cost slipcase-desktop
    // its Dock icon for nine days.** `eframe`'s `epi_integration.rs` substitutes
    // its own `data/icon.png` — the egui logo — for any viewport that names no
    // icon, and `app_icon.rs` hands that to `-[NSApplication
    // setApplicationIconImage:]`, which outranks the bundle's `.icns`. Finder
    // and every API still resolve the right drawing, so nothing short of a
    // person looking at the Dock finds it. An empty `IconData` is how the icon
    // is declined rather than replaced: `AppTitleIconSetter::new` turns one into
    // `None`, and the macOS arm only calls `setApplicationIconImage:` where
    // there is an image.
    //
    #[cfg(target_os = "macos")]
    let viewport = viewport.with_icon(egui::IconData::default());

    // Windows is the opposite case: it takes a window icon from a resource
    // compiled into the executable, and `build.rs` keeps a resource compiler out
    // of this build, so nothing is there to find. The `.ico` is carried as bytes
    // and one entry decoded at startup instead. Shadowed rather than made
    // mutable, so no platform without an icon to set carries an unused `mut`.
    #[cfg(target_os = "windows")]
    let viewport = match window_icon() {
        Some(icon) => viewport.with_icon(icon),
        None => viewport,
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    // "Filebase" rather than the crate name: the product name goes in front of
    // a person and `filebase` stays on disk and on PATH.
    eframe::run_native(
        "Filebase",
        options,
        Box::new(move |cc| {
            // Where the toolkit will not say whether the desktop is light or
            // dark, ask the desktop. Empty on the two platforms where `winit`
            // answers; on Linux it reads the portal and then follows it.
            system_theme::follow(&cc.egui_ctx);

            let mut app = App::new();
            if let Some(folder) = opened {
                app.bind(folder, &cc.egui_ctx);
            }
            Ok(Box::new(app))
        }),
    )
}

/// A folder dialog open on another thread.
struct Picking {
    /// The one message the thread sends when the dialog closes.
    answer: mpsc::Receiver<Option<PathBuf>>,
}

struct App {
    /// The directory every query is bound to, once one has been chosen.
    ///
    /// The grammar always requires `from`, and this is the window's half of the
    /// same arrangement the command's prompt has: the directory is bound here
    /// and supplied to the parser, so the box can leave `from` out. A `from`
    /// typed into the box still wins, which is the parser's rule and not this
    /// one's.
    folder: Option<PathBuf>,
    /// Whether the scan descends into subdirectories.
    recursive: bool,
    /// What is in the query box.
    query: String,
    /// The running or finished query, and its rows.
    run: Option<Run>,
    /// Why the last query would not parse, where it would not.
    refused: Option<String>,
    /// The selected row's path, relative to the folder, and what it opened to.
    ///
    /// The path is kept beside the detail so that a selection survives a rerun
    /// that returns the same container: the row indices move, the path does
    /// not.
    selected: Option<Detail>,
    /// Where a payload is put before it is handed over: a directory of this
    /// process's own, made on the first Open and removed with its contents when
    /// this drops.
    ///
    /// The application never writes beside the container it is reading.
    scratch: Option<tempfile::TempDir>,
    /// What the last attempt to hand a payload over said, where it had
    /// something to say.
    said: Option<String>,
    /// A dialog open on another thread.
    picking: Option<Picking>,
}

impl App {
    fn new() -> Self {
        Self {
            folder: None,
            recursive: false,
            query: FIRST_QUERY.to_owned(),
            run: None,
            refused: None,
            selected: None,
            scratch: None,
            said: None,
            picking: None,
        }
    }

    /// Bind to a folder and run the first query against it.
    ///
    /// A path that is not a directory is refused here and said so in the window,
    /// because the alternative is a window bound to something it cannot scan
    /// that only says so once somebody presses Run. `slipql::execute` would
    /// refuse it too; this refuses it in the same words the query bar uses for
    /// every other refusal.
    fn bind(&mut self, folder: PathBuf, ctx: &egui::Context) {
        if !folder.is_dir() {
            self.refused = Some(fill(
                t("{path} is not a folder."),
                &[("path", &folder.display().to_string())],
            ));
            return;
        }
        self.folder = Some(folder);
        self.selected = None;
        self.run_query(ctx);
    }

    /// Parse what is in the box and start scanning.
    ///
    /// Replacing `self.run` is what cancels a scan already going: the old
    /// receiver drops with it, the worker's next send fails, and it stops
    /// reading the tree for an answer nobody will see.
    fn run_query(&mut self, ctx: &egui::Context) {
        let Some(folder) = self.folder.clone() else {
            return;
        };
        self.said = None;
        let source = slipql::ast::Source {
            root: folder,
            recursive: self.recursive,
        };
        let query = match slipql::parse_with(&self.query, Some(&source)) {
            Ok(query) => query,
            Err(e) => {
                self.refused = Some(e.to_string());
                self.run = None;
                return;
            }
        };
        self.refused = None;
        let repaint = {
            let ctx = ctx.clone();
            move || ctx.request_repaint()
        };
        match Run::start(&query, repaint) {
            Ok(run) => {
                self.run = Some(run);
                self.selected = None;
            }
            Err(e) => {
                self.refused = Some(e.to_string());
                self.run = None;
            }
        }
    }

    /// Open a folder dialog on a thread of its own.
    ///
    /// `rfd`'s blocking dialog blocks the thread it is called on, which here is
    /// the one drawing the window, so it goes somewhere else and answers
    /// through a channel the next frame reads.
    fn pick_folder(&mut self) {
        if self.picking.is_some() {
            return;
        }
        let (sender, answer) = mpsc::channel();
        let start = self.folder.clone();
        std::thread::Builder::new()
            .name("filebase-dialog".to_owned())
            .spawn(move || {
                let mut dialog = rfd::FileDialog::new().set_title(t("Choose a folder"));
                if let Some(start) = start {
                    dialog = dialog.set_directory(start);
                }
                let _ = sender.send(dialog.pick_folder());
            })
            .ok();
        self.picking = Some(Picking { answer });
    }

    /// Hand the selected container's payload to whatever the system opens it
    /// with.
    ///
    /// The payload goes into this process's own directory first. Asking the
    /// platform to open a member of a ZIP archive is not a thing any of the
    /// three platforms can do, and extracting beside the container would change
    /// the directory this window was asked to look at.
    fn open_payload(&mut self) {
        let Some(detail) = &self.selected else { return };
        let scratch = match &self.scratch {
            Some(dir) => dir,
            None => {
                // 0700 asked for explicitly. `Builder::tempdir` goes through
                // the umask, which is 0755 under the common one and 0775 under
                // Debian's, and slipcase-desktop shipped that way for a while:
                // every payload somebody pressed Open on was readable by any
                // account on the machine.
                match new_scratch() {
                    Ok(dir) => self.scratch.insert(dir),
                    Err(e) => {
                        self.said = Some(fill(
                            t("Cannot make somewhere to put the payload: {why}"),
                            &[("why", &e.to_string())],
                        ));
                        return;
                    }
                }
            }
        };
        match detail.extract_to(scratch.path()) {
            Ok(out) => {
                if let Err(e) = opener::open(&out) {
                    self.said = Some(fill(
                        t("Nothing opened it: {why}"),
                        &[("why", &e.to_string())],
                    ));
                }
            }
            Err(e) => {
                self.said = Some(fill(
                    t("Cannot read the payload: {why}"),
                    &[("why", &e.to_string())],
                ));
            }
        }
    }
}

/// A directory of this process's own, readable by nobody else.
fn new_scratch() -> std::io::Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix("filebase-");
    // Asked for rather than taken: `Builder::tempdir` goes through the umask,
    // which leaves 0755 under the common one and 0775 under Debian's. Not a
    // question on Windows, where a directory under the user's own temporary
    // path is not readable by other accounts to begin with.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        builder.permissions(std::fs::Permissions::from_mode(0o700));
    }
    builder.tempdir()
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if let Some(picking) = &self.picking {
            if let Ok(answer) = picking.answer.try_recv() {
                self.picking = None;
                if let Some(folder) = answer {
                    self.bind(folder, &ctx);
                }
            }
        }

        // Once a frame, and never blocking. The columns of `select *` grow as
        // rows arrive, so the header is drawn from whatever the run says its
        // columns are now rather than from what they were when it started.
        if let Some(run) = &mut self.run {
            match run.drain() {
                // Nothing to do beyond having taken them: the table draws what
                // is there and the frame is already happening. Matched
                // exhaustively rather than ignored so that an arm added to
                // `Update` is a compile error here.
                Update::Nothing | Update::Rows | Update::Finished => {}
            }
        }

        egui::Panel::top("folder").show(ui, |ui| self.folder_bar(ui, &ctx));
        egui::Panel::top("query").show(ui, |ui| self.query_bar(ui, &ctx));
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::right("detail")
            .resizable(true)
            // Wide enough for the tree rather than for the payload card. The
            // tree spends about 210 pixels on the key column before a value
            // starts, and a nested key pushes its value another 25 right, so a
            // pane of 360 leaves a metadata string about 140 pixels to be read
            // in and clips most of them. 480 leaves it 235, which fits the
            // longest thing a description usually holds — a payload filename.
            // Measured off the store frames, where a clipped value is what the
            // shot is of.
            .default_size(480.0)
            .show(ui, |ui| self.detail_pane(ui));
        egui::CentralPanel::default().show(ui, |ui| self.results(ui));
    }
}

impl App {
    fn folder_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button(t("Open folder…")).clicked() {
                self.pick_folder();
            }
            let recursive = ui
                .checkbox(&mut self.recursive, t("recursive"))
                .on_hover_text(t("Descend into subdirectories."));
            if recursive.changed() {
                self.run_query(ctx);
            }
            match &self.folder {
                // `display()` and not a lossy conversion: a path that is not
                // UTF-8 is still a path somebody chose, and the bar is the one
                // place it is named back to them.
                Some(folder) => {
                    ui.label(folder.display().to_string());
                }
                None => {
                    ui.weak(t("No folder chosen."));
                }
            }
        });
        ui.add_space(4.0);
    }

    fn query_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let run = ui.button(t("Run"));
            let bound = self.folder.is_some();
            // Disabled without a folder rather than hidden: the box is where a
            // person reads what the language looks like, and an empty window
            // that shows nothing to type teaches nothing.
            let box_ = ui.add_enabled(
                bound,
                egui::TextEdit::singleline(&mut self.query)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .hint_text(FIRST_QUERY),
            );
            let entered = box_.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if bound && (run.clicked() || entered) {
                self.run_query(ctx);
                // Enter runs the query and leaves the caret where it was, so a
                // correction is a keystroke rather than a click back into the
                // box.
                box_.request_focus();
            }
        });
        if let Some(refused) = &self.refused {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, refused);
            });
        }
        ui.add_space(4.0);
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            match &self.run {
                Some(run) => ui.label(run.status()),
                None => ui.weak(t("No query has been run.")),
            };
        });
        if let Some(said) = &self.said {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(ui.visuals().error_fg_color, said);
            });
        }
        // Everything the scan noticed, under the count rather than beside it:
        // a skipped file and a comparison across types are not errors, the
        // query still answered, and a person reads them after the rows the way
        // the command prints them after the rows.
        if let Some(notices) = self.run.as_ref().and_then(Run::notices) {
            if notices.lines() > 0 {
                egui::CollapsingHeader::new(t("What the scan noticed"))
                    .id_salt("notices")
                    .show(ui, |ui| {
                        for line in notices.skipped.iter().chain(&notices.mismatches) {
                            ui.weak(line);
                        }
                    });
            }
        }
        ui.add_space(4.0);
    }

    fn results(&mut self, ui: &mut egui::Ui) {
        let Some(run) = &self.run else {
            ui.centered_and_justified(|ui| {
                ui.weak(t("Choose a folder, then run a query."));
            });
            return;
        };
        if run.columns().is_empty() && run.rows().is_empty() {
            ui.centered_and_justified(|ui| {
                ui.weak(if run.is_running() {
                    t("Scanning…")
                } else {
                    t("Nothing matched.")
                });
            });
            return;
        }

        // The selection is made here and applied after the table, because
        // opening a container borrows `self` and the table already has it.
        let mut chosen: Option<String> = None;
        let selected = self.selected.as_ref().map(|d| d.relative.clone());

        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("results")
                .striped(true)
                .num_columns(run.columns().len())
                .show(ui, |ui| {
                    for column in run.columns() {
                        ui.strong(column);
                    }
                    ui.end_row();
                    for row in run.rows() {
                        let is_selected = selected.as_deref() == Some(row.path.as_str());
                        for column in run.columns() {
                            // Every cell of the row is selectable and selects
                            // the whole row. A `Grid` has no row to click —
                            // there is no widget spanning one — so the row is
                            // made of cells that all do the same thing, which
                            // also gives the whole row the selected background
                            // rather than one cell of it.
                            if ui
                                .selectable_label(is_selected, query::cell(row, column))
                                .clicked()
                            {
                                chosen = Some(row.path.clone());
                            }
                        }
                        ui.end_row();
                    }
                });
        });

        if let Some(relative) = chosen {
            if let Some(folder) = self.folder.clone() {
                self.said = None;
                self.selected = Some(Detail::open(&folder, &relative));
            }
        }
    }

    fn detail_pane(&mut self, ui: &mut egui::Ui) {
        // **A resizable panel collapses to its minimum unless its content
        // claims the width**, which egui's own `Panel::resizable` documentation
        // says and which this pane proved: with nothing selected it drew one
        // short label, so the panel rendered 96 pixels — `Panel`'s floor — where
        // `default_size(360.0)` had asked for 360. Found by measuring the first
        // reference frame rather than by looking at the window, because at a
        // glance a narrow pane reads as a design choice.
        ui.take_available_width();

        let Some(detail) = &self.selected else {
            ui.add_space(8.0);
            ui.weak(t("Select a row."));
            return;
        };

        ui.add_space(8.0);
        ui.strong(detail.relative.clone());
        ui.add_space(8.0);

        let can_open = detail.payload().is_some_and(filebase::detail::Payload::can_be_opened);
        match &detail.outcome {
            Outcome::Unreadable(why) => {
                ui.colored_label(ui.visuals().error_fg_color, why);
                return;
            }
            Outcome::Read(contents) => {
                ui.label(t("Payload"));
                ui.label(contents.payload.name.clone());
                ui.weak(contents.payload.size_line());
                if let Some(why) = &contents.payload.unreadable {
                    ui.weak(fill(
                        t("This build cannot decode it: {why}"),
                        &[("why", why)],
                    ));
                }
            }
        }

        ui.add_space(6.0);
        // Offered only where there is a decoder, which is the difference
        // between a button that is not there and a button that does not work.
        if ui
            .add_enabled(can_open, egui::Button::new(t("Open payload")))
            .clicked()
        {
            self.open_payload();
        }
        ui.add_space(8.0);
        ui.separator();
        ui.label(t("Metadata"));

        // Borrowed again rather than held across the button above, because
        // `open_payload` takes `self` and the tree takes the document mutably.
        let Some(detail) = &mut self.selected else {
            return;
        };
        let Outcome::Read(contents) = &mut detail.outcome else {
            return;
        };
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                // The widget edits, and `ReadOnly` is what asks it for the
                // reading half of itself. There is no save under this pane and
                // there is no path through this application that writes a
                // container.
                flyleaf::render(ui, &mut contents.metadata, &ReadOnly);
            });
    }
}

#[cfg(test)]
mod tests {
    use super::CATALOGUES;

    /// Would catch the catalogue list and the `po/` directory drifting apart:
    /// a language named here with no file behind it does not compile, and a
    /// file with no entry here is a translation nothing can reach.
    #[test]
    fn german_is_one_of_the_catalogues() {
        assert!(CATALOGUES.iter().any(|(tag, _)| *tag == "de"));
    }

    /// Would catch a catalogue that parses to nothing — the shape a
    /// mis-encoded or truncated `.po` takes, which `msgfmt --check` cannot see
    /// because it reads the file rather than what the program makes of it.
    ///
    /// The lookup goes through `filebase::i18n`, which is the crate-wide
    /// catalogue every sentence in the window is drawn from, so this is the
    /// same path a person running the application in German takes.
    #[test]
    fn the_german_catalogue_answers() {
        assert_eq!(
            filebase::i18n::set_language("de", CATALOGUES).as_deref(),
            Some("de")
        );
        assert_eq!(filebase::i18n::t("Payload"), "Nutzlast");
        assert_eq!(filebase::i18n::t("Open payload"), "Nutzlast öffnen");
        // A message with a placeholder: the braces have to survive the
        // translation, because `fill` looks them up by name afterwards.
        assert!(filebase::i18n::t("{path} is not a folder.").contains("{path}"));
    }
}

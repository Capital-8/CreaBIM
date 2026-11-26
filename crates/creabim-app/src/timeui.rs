//! Project time on screen (Mockup_Project_Time_REV02, design A): the time
//! pill in the status bar and its panel, the Time area (Stages, Entries,
//! Week) and Time and fees in the Project tab. The bookkeeping is in
//! `projecttime.rs`.

use super::*;
use crate::projecttime::{self as pt, Activity, ClockState, Entry, Scheme, TimeBook, Tracking};

/// The Time area's three ways of showing the book.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TimeView {
    #[default]
    Stages,
    Entries,
    Week,
}

/// Project time between frames.
#[derive(Debug, Default)]
pub struct TimeUi {
    pub book: TimeBook,
    pub clock: pt::Clock,
    /// The project file the book belongs to.
    path: String,
    /// Changed since it was last written beside the project.
    dirty: bool,
    last_store: f64,
    /// When egui last saw the mouse or a key (where the system can't say).
    last_input: f64,
    /// The computer's offset from UTC, asked again every minute.
    pub tz: i32,
    tz_at: f64,
    pub view: TimeView,
    /// The entry opened for editing in the Entries view.
    pub picked: Option<usize>,
    /// The stage opened for editing in the Stages view.
    pub stage_edit: Option<usize>,
    /// The card width of the Time area this frame (the area's own).
    pub w: f32,
    /// The day unfolded in the Week view.
    pub week_open: Option<i64>,
    /// The entry shown lit in Notes › Work log (come from Entries).
    pub log_hot: Option<usize>,
    /// Weeks back from this one, in the Week view.
    pub week_back: i64,
    /// A problem writing the book, said once.
    told: bool,
}

/// How often a changed book is written while time counts.
const STORE_EVERY_S: f64 = 30.0;

impl CreaBimApp {
    /// The person time is signed with: the name set in Time and fees, else
    /// the computer's user name.
    pub(super) fn time_person(&self) -> String {
        let s = self.shell.settings.time_name.trim();
        if !s.is_empty() {
            return s.to_string();
        }
        std::env::var("USERNAME")
            .or_else(|_| std::env::var("USER"))
            .unwrap_or_else(|_| "Me".to_string())
    }

    /// What the work now is, from the area in front and the tool in hand.
    fn time_activity(&self) -> Activity {
        use crate::areas::Shows;
        match self.areas.root.areas().get(self.areas.focus) {
            Some(Shows::Model) | Some(Shows::Profiles) => Activity::Modelling,
            Some(Shows::View(v)) => {
                if self.is_sheet(*v) {
                    Activity::Sheets
                } else if self.plan.tool == Tool::Select || self.plan.tool.is_annotation() {
                    Activity::Drawing
                } else {
                    Activity::Modelling
                }
            }
            Some(Shows::Masters) => Activity::Sheets,
            _ => Activity::Other,
        }
    }

    /// Each frame: follow the project open, count, and keep the book.
    pub(super) fn time_frame(&mut self, ctx: &egui::Context) {
        let now = pt::now_s();
        if self.time.path != self.path {
            self.time_switch(now);
        }
        if now - self.time.tz_at > 60.0 {
            self.time.tz = pt::local_offset_min();
            self.time.tz_at = now;
        }
        let saw = ctx.input(|i| {
            !i.events.is_empty() || i.pointer.delta() != egui::Vec2::ZERO || i.pointer.any_down()
        });
        if saw {
            self.time.last_input = now;
        }
        let focused = pt::app_in_front().unwrap_or_else(|| ctx.input(|i| i.focused));
        let input_age = pt::input_age_s().unwrap_or(now - self.time.last_input);
        let person = self.time_person();
        let m = pt::Moment {
            now,
            tz_min: self.time.tz,
            focused,
            input_age,
            activity: self.time_activity(),
            person: &person,
        };
        if self.time.book.tick(&mut self.time.clock, &m) {
            self.time.dirty = true;
        }
        if self.time.dirty && now - self.time.last_store > STORE_EVERY_S {
            self.time_store();
        }
        if self.time.book.tracking != Tracking::Off {
            // the pill's seconds, and the idle check, while nothing moves
            ctx.request_repaint_after(std::time::Duration::from_millis(1000));
        }
    }

    /// Another project is open: keep the last one's book, read this one's.
    fn time_switch(&mut self, now: f64) {
        let was = std::mem::take(&mut self.time.path);
        let was_on_disk = !was.is_empty() && std::path::Path::new(&was).exists();
        if was_on_disk {
            self.time_store_at(&was);
        }
        self.time.clock.close();
        self.time.picked = None;
        self.time.path = self.path.clone();
        // a project saved for the first time keeps what was counted
        // before it had a file
        let carry = !was_on_disk && !self.time.book.entries.is_empty() && !was.is_empty();
        if !carry {
            let standard = creabim_core::standards::of(self.doc.model()).name;
            self.time.book = pt::load(&self.path)
                .unwrap_or_else(|| TimeBook::new(Scheme::for_standard(&standard)));
            self.time.dirty = false;
        } else {
            self.time.dirty = true;
        }
        self.time.last_store = now;
    }

    /// The project was just saved, perhaps under a new name (Save as, or
    /// its first save): its time goes with it.
    pub(super) fn time_frame_path_sync(&mut self) {
        if self.time.path == self.path {
            return;
        }
        let was = std::mem::take(&mut self.time.path);
        if !was.is_empty() && std::path::Path::new(&was).exists() {
            self.time_store_at(&was);
        }
        self.time.path = self.path.clone();
        self.time.dirty = true;
    }

    /// Write the book beside the project now.
    pub(super) fn time_store(&mut self) {
        let path = self.time.path.clone();
        self.time_store_at(&path);
    }

    fn time_store_at(&mut self, project: &str) {
        self.time.last_store = pt::now_s();
        match pt::store(project, &self.time.book) {
            Ok(()) => {
                self.time.dirty = false;
                self.time.told = false;
            }
            Err(e) => {
                if !self.time.told {
                    self.status = format!("Couldn't keep the project's time beside it: {e}");
                    self.time.told = true;
                }
            }
        }
    }

    pub(super) fn time_changed(&mut self) {
        self.time.dirty = true;
        self.time.last_store = 0.0;
    }

    /// Today's working, open and idle seconds.
    fn time_today(&self) -> [f64; 3] {
        self.time.book.today(pt::now_s() as i64, self.time.tz)
    }

    fn time_stage_now_use(&self) -> Option<f64> {
        self.time.book.stage_use(self.time.book.stage_now)
    }

    /// The pill in the status bar, while time is kept for the project.
    pub(super) fn time_pill_json(&self) -> Option<serde_json::Value> {
        let b = &self.time.book;
        if b.tracking == Tracking::Off {
            return None;
        }
        let today = self.time_today()[0];
        let stage = b.stage().map_or("", |s| s.short()).to_string();
        let (text, cls) = match self.time.clock.state {
            ClockState::Counting => {
                let budget = match pt::budget_state(self.time_stage_now_use()) {
                    pt::Budget::Fine => "",
                    pt::Budget::Near => " warn",
                    pt::Budget::Over => " over",
                };
                (pt::clock(today), format!("run{budget}"))
            }
            ClockState::Idle => ("Idle · paused".to_string(), "idle".to_string()),
            ClockState::Away => (format!("Away · {}", pt::clock(today)), "away".to_string()),
            ClockState::Paused if b.tracking == Tracking::Manual && !self.time.clock.started => {
                ("Start the clock".to_string(), "paused".to_string())
            }
            _ => (
                format!("Paused · {}", pt::clock(today)),
                "paused".to_string(),
            ),
        };
        let tip = match self.time.clock.state {
            ClockState::Counting => "Project time is counting: today's working time and the stage it goes to. Click to pause, switch stage or add a note.",
            ClockState::Idle => "No mouse or key for a while: nothing counts until you come back.",
            ClockState::Away => "Another program is in front: shown as open time, not billed.",
            _ => "The clock is paused. Click to resume.",
        };
        Some(serde_json::json!({
            "text": text,
            "stage": stage,
            "cls": cls,
            "pulse": b.pulse,
            "hint": tip,
        }))
    }

    /// The strip at the browser's foot while the card is pinned (MU101).
    pub(super) fn time_dock_json(&self) -> Option<serde_json::Value> {
        if !self.shell.settings.time_pinned {
            return None;
        }
        let pill = self.time_pill_json()?;
        let p = self.time_popup_json();
        let stage = self
            .time
            .book
            .stage()
            .map_or(String::new(), |s| s.name.clone());
        let used = p["stage_line"]
            .as_str()
            .unwrap_or("")
            .trim_start_matches("This stage · ")
            .to_string();
        Some(serde_json::json!({
            "clock": p["clock"], "running": p["running"], "manual": p["manual"],
            "cls": pill["cls"], "pulse": pill["pulse"],
            "line": if used.is_empty() { stage } else { format!("{stage} · {used}") },
            "use": p["use"], "budget": p["budget"],
        }))
    }

    /// The pill's panel (the "time" popup).
    pub(super) fn time_popup_json(&self) -> serde_json::Value {
        let b = &self.time.book;
        let now = pt::now_s() as i64;
        let [work, open, idle] = b.today(now, self.time.tz);
        let i = b.stage_now;
        let used = b.stage_hours(i);
        let stage_line = match b.stages.get(i) {
            Some(s) if s.budget_h > 0.0 => {
                format!("This stage · {} of {} h", pt::hm(used), fmt_h(s.budget_h))
            }
            Some(_) => format!("This stage · {} · no budget set", pt::hm(used)),
            None => String::new(),
        };
        let use_ = b.stage_use(i);
        let running = matches!(
            self.time.clock.state,
            ClockState::Counting | ClockState::Idle | ClockState::Away
        );
        let project = self
            .doc
            .model()
            .project_info()
            .map(|(_, p)| p.name.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| file_stem(&self.path));
        serde_json::json!({
            "project": project,
            "clock": pt::clock(work),
            "running": running,
            "manual": b.tracking == Tracking::Manual,
            "stages": b.stages.iter().enumerate().map(|(k, s)| serde_json::json!({
                "label": s.name, "on": k == i, "done": s.done,
            })).collect::<Vec<_>>(),
            "stage_line": stage_line,
            "use": use_.map(|u| (u * 100.0).min(100.0)),
            "budget": match pt::budget_state(use_) {
                pt::Budget::Fine => "",
                pt::Budget::Near => "warn",
                pt::Budget::Over => "over",
            },
            "today": format!(
                "Today {} working · {} open · {} idle",
                pt::hm(work / 3600.0),
                pt::hm(open / 3600.0),
                pt::hm(idle / 3600.0)
            ),
            "note": self.time.clock.open.and_then(|k| b.entries.get(k)).map(|e| e.note.clone()),
            "pinned": self.shell.settings.time_pinned,
        })
    }

    /// The view the Time area shows, as its header names it.
    pub(super) fn time_view_name(&self) -> &'static str {
        match self.time.view {
            TimeView::Stages => "Stages",
            TimeView::Entries => "Entries",
            TimeView::Week => "Week",
        }
    }

    /// The Time area's header: a manual entry and the timesheet (the views
    /// are its name's menu).
    pub(super) fn time_controls(&self) -> Vec<serde_json::Value> {
        use serde_json::json;
        let mut out = vec![];
        if self.time.book.tracking != Tracking::Off {
            out.push(json!({"kind": "button", "id": "time.manual", "icon": "plus", "label": "Manual entry",
                "hint": "Time spent outside CreaBIM: a site visit, a meeting, a call"}));
            out.push(json!({"kind": "button", "id": "time.csv", "icon": "download", "label": "Timesheet CSV",
                "hint": "Every entry with its stage, person and activity split, for a spreadsheet or your books"}));
            out.push(json!({"kind": "button", "id": "time.pdf", "icon": "file-text", "label": "Timesheet PDF",
                "hint": "A timesheet to send the client: the entries, each stage's hours and fee, the totals"}));
        }
        out
    }

    /// The Time area's views, for its header's menu.
    pub(super) fn time_view_items(&self) -> Vec<crate::ui_model::Item> {
        use crate::ui_model::Item;
        let v = self.time.view;
        vec![
            Item::new("time.view:stages", "Stages")
                .checked(v == TimeView::Stages)
                .hint("Each work stage against its fee and hours budget"),
            Item::new("time.view:entries", "Entries")
                .checked(v == TimeView::Entries)
                .hint("Every stretch of work, counted or typed in"),
            Item::new("time.view:week", "Week")
                .checked(v == TimeView::Week)
                .hint("Hours per day, by stage"),
        ]
    }

    /// The pill, its panel and the Time area's buttons. False for anything
    /// else.
    pub(super) fn time_action(&mut self, ctx: &egui::Context, head: &str, arg: &str) -> bool {
        match head {
            "time.pause" => {
                let b = &self.time.book;
                if b.tracking == Tracking::Manual && !self.time.clock.started {
                    self.time.clock.started = true;
                    self.time.clock.paused = false;
                    self.status = "The clock is counting.".into();
                } else {
                    self.time.clock.paused = !self.time.clock.paused;
                    self.status = if self.time.clock.paused {
                        "The clock is paused.".into()
                    } else {
                        "The clock is counting again.".into()
                    };
                }
            }
            "time.stage" => {
                if let Ok(i) = arg.parse::<usize>() {
                    if i < self.time.book.stages.len() && i != self.time.book.stage_now {
                        self.time.book.stage_now = i;
                        self.time.clock.close();
                        self.time_changed();
                        self.status =
                            format!("Time now goes to {}.", self.time.book.stages[i].name);
                    }
                }
            }
            "time.view" => {
                self.time.view = match arg {
                    "entries" => TimeView::Entries,
                    "week" => TimeView::Week,
                    _ => TimeView::Stages,
                };
                self.time_show_area();
            }
            "time.area" => {
                self.time_show_area();
            }
            "time.pin" => {
                // pinned, the card becomes a strip at the browser's foot
                let pin = !self.shell.settings.time_pinned;
                self.shell.settings.time_pinned = pin;
                self.shell.settings.store();
                self.close_popup();
                self.status = if pin {
                    "Project time is pinned at the foot of the Project browser. Its pin unpins it."
                        .into()
                } else {
                    "Project time is unpinned: the pill in the status bar opens it.".into()
                };
            }
            "time.note" => {
                // the entry counting now, opened in Entries to write in
                self.time.view = TimeView::Entries;
                self.time.picked = self.time.clock.open;
                self.time_show_area();
            }
            "time.on" => {
                self.time.book.tracking = Tracking::Auto;
                self.time_changed();
                self.status =
                    "Project time is on: it counts while you work in this project.".into();
            }
            "time.manual" => self.time_add_manual(),
            "time.csv" => self.time_export(ctx, false),
            "time.pdf" => self.time_export(ctx, true),
            _ => return false,
        }
        true
    }

    /// Set the note of the entry counting now (typed in the pill's panel).
    pub(super) fn time_set_note(&mut self, text: &str) {
        if let Some(e) = self
            .time
            .clock
            .open
            .and_then(|k| self.time.book.entries.get_mut(k))
        {
            e.note = text.trim().to_string();
            self.time_changed();
        }
    }

    /// Show the Time area: in the focused area, unless one shows it.
    pub(super) fn time_show_area(&mut self) {
        use crate::areas::Shows;
        if let Some(i) = self
            .areas
            .root
            .areas()
            .iter()
            .position(|s| *s == Shows::Time)
        {
            self.areas.focus = i;
            return;
        }
        // in the area in front, as picking Time in its header's menu does
        let i = self.areas.focus;
        if let Some(s) = self.areas.root.get_mut(i) {
            *s = Shows::Time;
        }
    }

    fn time_add_manual(&mut self) {
        let now = pt::now_s() as i64;
        // an hour ending now, on the stage now
        let person = self.time_person();
        // never across midnight: just after it, the hour starts at midnight
        let tz = self.time.tz;
        let midnight = pt::local_day(now, tz) * 86_400 - i64::from(tz) * 60;
        let start = (now - 3600).max(midnight);
        let mut e = Entry::manual(start, tz, self.time.book.stage_now, &person);
        e.note = "Typed in".into();
        self.time.book.entries.push(e);
        self.time.picked = Some(self.time.book.entries.len() - 1);
        self.time.view = TimeView::Entries;
        self.time_changed();
        self.status =
            "A typed-in hour added: set its day, times, stage and note in Entries.".into();
    }

    fn time_export(&mut self, _ctx: &egui::Context, pdf: bool) {
        let project = self
            .doc
            .model()
            .project_info()
            .map(|(_, p)| p.name.clone())
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| file_stem(&self.path));
        let client = self
            .doc
            .model()
            .project_info()
            .map(|(_, p)| p.client.clone())
            .unwrap_or_default();
        let (ext, what) = if pdf { ("pdf", "PDF") } else { ("csv", "CSV") };
        let start = format!(
            "{} timesheet {}.{ext}",
            project,
            pt::date_iso(pt::now_s() as i64, self.time.tz)
        );
        let start = std::path::Path::new(&self.path)
            .parent()
            .map(|d| d.join(&start).display().to_string())
            .unwrap_or(start);
        let Some(path) =
            crate::files::pick_save(&start, &format!("Save the timesheet as {what}"), ext)
        else {
            return;
        };
        let bytes = if pdf {
            self.time.book.pdf(&project, &client)
        } else {
            self.time.book.csv().into_bytes()
        };
        match std::fs::write(&path, bytes) {
            Ok(()) => {
                self.status = format!("Timesheet saved: {path}");
                self.last_export = Some(self.status.clone());
                self.last_export_dir = std::path::Path::new(&path)
                    .parent()
                    .map(|p| p.to_path_buf());
            }
            Err(e) => self.status = format!("Couldn't save the timesheet: {e}"),
        }
    }

    /// Show entry `i` in Notes › Work log, lit and scrolled to.
    pub(super) fn time_open_in_notes(&mut self, i: usize, ctx: &egui::Context) {
        self.time.log_hot = Some(i);
        ctx.data_mut(|d| d.insert_temp(egui::Id::new("time-worklog-scroll"), i));
        self.open_notes(super::notesui::WORKLOG);
    }

    // ---- the Time area ----

    pub(super) fn time_off(&mut self, ui: &mut egui::Ui) {
        let pal = self.shell.theme.palette();
        ui.add_space(24.0);
        ui.vertical_centered(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(36.0, 36.0), egui::Sense::hover());
            crate::svgicon::paint(ui.painter(), r, "clock", pal.muted);
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Project time is off for this project").strong().size(14.0));
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "Switched on, it counts the time you work in this project, splits it by work stage and by what you were doing, and shows it against each stage's fee and hours budget. It stays on this computer, beside the project file: never sent with the project, never in IFC.",
                    )
                    .color(pal.muted),
                )
                .wrap(),
            );
            ui.add_space(10.0);
            if ui
                .add(egui::Button::new("  Switch it on  ").min_size(egui::vec2(140.0, 26.0)))
                .on_hover_text("Counts while you work in the project, with idle after 5 minutes")
                .clicked()
            {
                let ctx = ui.ctx().clone();
                self.time_action(&ctx, "time.on", "");
            }
        });
    }

    // ---- Time and fees, in the Project tab ----

    pub(super) fn time_props(&mut self, ui: &mut egui::Ui) {
        let open = self.prop_header(
            ui,
            "project_time",
            "Time and fees",
            false,
            Some("project-time"),
        );
        let mut changed = false;
        let mut name = self.shell.settings.time_name.clone();
        crate::props::fold(ui, open, |ui| {
            let b = &mut self.time.book;
            // a list, not three buttons: the panel's narrowest cut them to …
            let mut tracking = b.tracking;
            let track_name = |t: Tracking| match t {
                Tracking::Off => "Off",
                Tracking::Auto => "Auto, while you work",
                Tracking::Manual => "Manual, after Start",
            };
            crate::props::row(ui, "Tracking", |ui| {
                egui::ComboBox::from_id_salt("time-tracking")
                    .icon(crate::props::combo_icon)
                    .width(ui.available_width())
                    .selected_text(track_name(tracking))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut tracking,
                            Tracking::Off,
                            track_name(Tracking::Off),
                        )
                        .on_hover_text("Nothing is counted for this project");
                        ui.selectable_value(
                            &mut tracking,
                            Tracking::Auto,
                            track_name(Tracking::Auto),
                        )
                        .on_hover_text("Counts while you work in the project; idle stops it");
                        ui.selectable_value(
                            &mut tracking,
                            Tracking::Manual,
                            track_name(Tracking::Manual),
                        )
                        .on_hover_text("Counts only after you press Start, in the time pill");
                    });
            });
            if tracking != b.tracking {
                b.tracking = tracking;
                changed = true;
            }
            if b.tracking != Tracking::Off {
                changed |= crate::props::check(ui, "Pulse", &mut b.pulse, "Dot beats")
                    .on_hover_text("The dot in the time pill beats while time counts")
                    .changed();
                crate::props::row(ui, "Idle after", |ui| {
                    changed |= crate::props::fill(
                        ui,
                        egui::DragValue::new(&mut b.idle_min)
                            .range(1..=60)
                            .suffix(" min"),
                    )
                    .on_hover_text(
                        "No mouse or key for this long stops the count until you come back",
                    )
                    .changed();
                });
                let mut scheme = b.scheme;
                crate::props::row(ui, "Stages", |ui| {
                    egui::ComboBox::from_id_salt("time-scheme")
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width())
                        .selected_text(scheme.label())
                        .show_ui(ui, |ui| {
                            for s in [Scheme::Riba, Scheme::Aia, Scheme::Own] {
                                ui.selectable_value(&mut scheme, s, s.label());
                            }
                        });
                });
                if scheme != b.scheme {
                    b.set_scheme(scheme);
                    changed = true;
                }
                let mut now = b.stage_now;
                crate::props::row(ui, "Stage now", |ui| {
                    egui::ComboBox::from_id_salt("time-stage-now")
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width())
                        .selected_text(b.stage().map_or("", |s| s.name.as_str()))
                        .show_ui(ui, |ui| {
                            for (k, s) in b.stages.iter().enumerate() {
                                ui.selectable_value(&mut now, k, &s.name);
                            }
                        });
                });
                if now != b.stage_now {
                    b.stage_now = now;
                    self.time.clock.close();
                    changed = true;
                }
                let b = &mut self.time.book;
                crate::props::row(ui, "Rate", |ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    changed |= crate::props::text_field(
                        ui,
                        egui::TextEdit::singleline(&mut b.currency).char_limit(4),
                        34.0,
                    )
                    .on_hover_text("The currency sign")
                    .changed();
                    changed |= crate::props::fill(
                        ui,
                        egui::DragValue::new(&mut b.rate)
                            .range(0.0..=100_000.0)
                            .speed(1.0)
                            .suffix(" / h")
                            .max_decimals(2),
                    )
                    .on_hover_text("Your hourly rate: what an hour is worth against a stage's fee")
                    .changed();
                });
                let mut round = b.rounding_min;
                crate::props::row(ui, "Rounding", |ui| {
                    let label = |m: u32| {
                        if m == 0 {
                            "None".to_string()
                        } else {
                            format!("{m} min on export")
                        }
                    };
                    egui::ComboBox::from_id_salt("time-rounding")
                        .icon(crate::props::combo_icon)
                        .width(ui.available_width())
                        .selected_text(label(round))
                        .show_ui(ui, |ui| {
                            for m in [6, 15, 30, 0] {
                                ui.selectable_value(&mut round, m, label(m));
                            }
                        });
                });
                if round != b.rounding_min {
                    b.rounding_min = round;
                    changed = true;
                }
                crate::props::row(ui, "You", |ui| {
                    crate::props::text_field(
                        ui,
                        egui::TextEdit::singleline(&mut name).hint_text(self.time_person()),
                        f32::INFINITY,
                    )
                    .on_hover_text("The name this computer signs its time with, so a studio sharing a project adds up correctly")
                });
                if crate::props::action_icon(
                    ui,
                    "",
                    "clock",
                    "Open the Time area",
                    "Stages, entries and the week, with the timesheet",
                    true,
                )
                .clicked()
                {
                    let ctx = ui.ctx().clone();
                    self.time_action(&ctx, "time.area", "");
                }
            }
            crate::props::info(
                ui,
                "Kept beside the project, private: never sent with the file or in IFC.",
                "project-time",
            );
        });
        if name != self.shell.settings.time_name {
            self.shell.settings.time_name = name;
            self.shell.settings.store();
        }
        if changed {
            self.time_changed();
        }
    }
}

/// The day, from and to of a typed-in entry. True when changed.
pub(super) fn manual_times(ui: &mut egui::Ui, e: &mut Entry, i: usize) -> bool {
    let id = ui.id().with(("time-manual", i));
    // the texts as typed, kept while they are being typed
    let (mut date, mut from, mut to) = ui
        .ctx()
        .data(|d| d.get_temp::<(String, String, String)>(id))
        .unwrap_or_else(|| {
            (
                pt::date_iso(e.start, e.tz_min),
                pt::clock_hm(e.start, e.tz_min),
                pt::clock_hm(e.end, e.tz_min),
            )
        });
    let mut edited = false;
    super::timeviews::trow(ui, 300.0, "Day", |ui| {
        edited |= crate::props::text_field(
            ui,
            egui::TextEdit::singleline(&mut date).hint_text("2026-11-18"),
            f32::INFINITY,
        )
        .changed();
    });
    super::timeviews::trow(ui, 300.0, "From, to", |ui| {
        let w = (ui.available_width() - 6.0) / 2.0;
        ui.spacing_mut().item_spacing.x = 6.0;
        edited |= crate::props::text_field(
            ui,
            egui::TextEdit::singleline(&mut from).hint_text("09:00"),
            w,
        )
        .changed();
        edited |= crate::props::text_field(
            ui,
            egui::TextEdit::singleline(&mut to).hint_text("10:30"),
            w,
        )
        .changed();
    });
    let mut changed = false;
    if edited {
        if let (Some(day), Some(a), Some(b)) = (
            pt::parse_date(&date),
            pt::parse_hm(&from),
            pt::parse_hm(&to),
        ) {
            let base = day * 86_400 - i64::from(e.tz_min) * 60;
            e.start = base + a;
            e.end = base + if b < a { b + 86_400 } else { b };
            e.fit_manual();
            changed = true;
        }
        ui.ctx().data_mut(|d| d.insert_temp(id, (date, from, to)));
    }
    changed
}

/// Hours as `44` or `12.5`.
fn fmt_h(h: f64) -> String {
    if (h - h.round()).abs() < 1e-9 {
        format!("{h:.0}")
    } else {
        format!("{h:.1}")
    }
}

fn file_stem(path: &str) -> String {
    std::path::Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Project".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::test_app;

    #[test]
    fn the_pill_shows_only_while_time_is_kept_and_its_panel_lists_the_stages() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        app.time_frame(&ctx);
        assert!(app.status_state().get("timer").is_none(), "off by default");
        assert!(app.time_action(&ctx, "time.on", ""));
        assert_eq!(app.time.book.tracking, Tracking::Auto);
        app.time_frame(&ctx);
        let t = &app.status_state()["timer"];
        assert!(t["text"].is_string(), "{t}");
        assert!(app.time_action(&ctx, "time.stage", "2"));
        assert_eq!(app.time.book.stage_now, 2);
        let p = app.time_popup_json();
        assert_eq!(
            p["stages"].as_array().unwrap().len(),
            app.time.book.stages.len()
        );
        assert_eq!(p["stages"][2]["on"], true);
        // Pause stops it, and the pill says so
        app.time_action(&ctx, "time.pause", "");
        app.time_frame(&ctx);
        assert_eq!(app.time.clock.state, ClockState::Paused);
        assert!(app.status_state()["timer"]["text"]
            .as_str()
            .unwrap()
            .starts_with("Paused"));
    }

    #[test]
    fn the_time_area_opens_in_front_and_takes_a_typed_in_hour() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        app.time_action(&ctx, "time.on", "");
        app.time_action(&ctx, "time.view", "week");
        assert_eq!(
            app.areas.root.areas().get(app.areas.focus),
            Some(&crate::areas::Shows::Time)
        );
        assert_eq!(app.time.view, TimeView::Week);
        app.time_action(&ctx, "time.manual", "");
        assert_eq!(app.time.book.entries.len(), 1);
        assert!(app.time.book.entries[0].manual);
        assert!((app.time.book.total_hours() - 1.0).abs() < 1e-9);
        assert_eq!(app.time.view, TimeView::Entries);
        assert!(app.time_controls().iter().any(|c| c["id"] == "time.csv"));
    }

    #[test]
    fn saving_keeps_the_time_beside_the_project_never_inside_it() {
        let mut app = test_app();
        let ctx = egui::Context::default();
        let dir = std::env::temp_dir().join(format!("creabim-time-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        app.path = dir.join("house.creabim").display().to_string();
        app.time_frame(&ctx);
        app.time_action(&ctx, "time.on", "");
        app.time_action(&ctx, "time.manual", "");
        app.save_file().unwrap();
        let beside = dir.join("house.creabim-time");
        assert!(beside.exists());
        let back = crate::projecttime::load(&app.path).unwrap();
        assert_eq!(back.entries.len(), 1);
        // Save as: the time goes with the project
        app.path = dir.join("house 2.creabim").display().to_string();
        app.save_file().unwrap();
        assert!(dir.join("house 2.creabim-time").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}

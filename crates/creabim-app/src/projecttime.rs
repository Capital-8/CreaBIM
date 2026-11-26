//! Project time (Mockup_Project_Time_REV02, design A, A on all five): what
//! the work on a project took, by work stage and by what was being done,
//! against each stage's fee and hours budget.
//!
//! Kept beside the project in a file of its own (`<name>.creabim-time`),
//! never inside the `.creabim` and never in an IFC export, so sending the
//! project to a client never sends the hours and rates (decision 3). It is
//! off until switched on for a project. Nothing here touches the network.
//!
//! This is the bookkeeping alone, free of the window: the app hands
//! [`TimeBook::tick`] a [`Moment`] each frame and draws what it returns.

use serde::{Deserialize, Serialize};

/// How time is counted for a project (the Tracking switch).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Tracking {
    /// Nothing is counted (a project starts here).
    #[default]
    Off,
    /// Counted while the project is open and being worked in (decision 2).
    Auto,
    /// Counted only after Start, as Focus Flow.
    Manual,
}

/// Where a project's stage list comes from (decision 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Scheme {
    /// The RIBA Plan of Work 2020 (UK and most A-sheet countries).
    #[default]
    Riba,
    /// The AIA phases (US).
    Aia,
    /// The architect's own list.
    Own,
}

impl Scheme {
    pub fn label(self) -> &'static str {
        match self {
            Scheme::Riba => "RIBA Plan of Work",
            Scheme::Aia => "AIA phases",
            Scheme::Own => "Own list",
        }
    }

    /// The scheme a project's drawing standard suggests.
    pub fn for_standard(name: &str) -> Scheme {
        if name == "US" {
            Scheme::Aia
        } else {
            Scheme::Riba
        }
    }

    /// Its stages, with no fee or budget yet.
    pub fn stages(self) -> Vec<Stage> {
        let names: &[&str] = match self {
            Scheme::Riba => &[
                "0 Strategic definition",
                "1 Preparation and briefing",
                "2 Concept design",
                "3 Spatial coordination",
                "4 Technical design",
                "5 Manufacturing and construction",
                "6 Handover",
                "7 Use",
            ],
            Scheme::Aia => &[
                "Pre-design",
                "Schematic design",
                "Design development",
                "Construction documents",
                "Bidding and negotiation",
                "Construction administration",
            ],
            Scheme::Own => &["Design", "Drawings", "Site"],
        };
        names.iter().map(|n| Stage::new(n)).collect()
    }
}

/// One work stage, with what it is paid and how long it should take.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stage {
    pub name: String,
    /// The stage's fee, in the book's currency; 0 when not set.
    pub fee: f64,
    /// Its hours budget; 0 when not set (no bar is drawn).
    pub budget_h: f64,
    /// Finished: shown as done, never offered as the stage now.
    pub done: bool,
}

impl Default for Stage {
    fn default() -> Self {
        Stage::new("Stage")
    }
}

impl Stage {
    pub fn new(name: &str) -> Stage {
        Stage {
            name: name.to_string(),
            fee: 0.0,
            budget_h: 0.0,
            done: false,
        }
    }

    /// Its name without the leading number: "Concept design".
    pub fn short(&self) -> &str {
        let n = self
            .name
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '–' || c == '-');
        let n = n.trim_start();
        if n.is_empty() {
            &self.name
        } else {
            n
        }
    }
}

/// What the hours were spent doing, worked out from the area and tool in
/// use (never typed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Activity {
    Modelling,
    Drawing,
    Sheets,
    Other,
}

impl Activity {
    pub const ALL: [Activity; 4] = [
        Activity::Modelling,
        Activity::Drawing,
        Activity::Sheets,
        Activity::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Activity::Modelling => "Modelling",
            Activity::Drawing => "Drawing",
            Activity::Sheets => "Sheets",
            Activity::Other => "Other",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// One stretch of work: counted, or typed in for work outside CreaBIM.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    /// Unix seconds.
    pub start: i64,
    pub end: i64,
    /// The computer's offset from UTC when it was counted, in minutes, so
    /// it keeps its day and hours after a clock change or a journey.
    pub tz_min: i32,
    pub stage: usize,
    pub person: String,
    /// Working time: CreaBIM in front and in use. What bills.
    pub working_s: f64,
    /// Open time: the project open with another program in front.
    pub open_s: f64,
    /// Idle: in front, but no mouse or key for the idle time.
    pub idle_s: f64,
    /// The working time split by [`Activity`].
    pub act_s: [f64; 4],
    pub note: String,
    /// Typed in, not counted.
    pub manual: bool,
}

impl Default for Entry {
    fn default() -> Self {
        Entry {
            start: 0,
            end: 0,
            tz_min: 0,
            stage: 0,
            person: String::new(),
            working_s: 0.0,
            open_s: 0.0,
            idle_s: 0.0,
            act_s: [0.0; 4],
            note: String::new(),
            manual: false,
        }
    }
}

impl Entry {
    /// A typed-in entry: an hour from `start`, all of it working time.
    pub fn manual(start: i64, tz_min: i32, stage: usize, person: &str) -> Entry {
        let mut e = Entry {
            start,
            end: start + 3600,
            tz_min,
            stage,
            person: person.to_string(),
            manual: true,
            ..Entry::default()
        };
        e.fit_manual();
        e
    }

    /// A typed-in entry's hours follow its start and end.
    pub fn fit_manual(&mut self) {
        if !self.manual {
            return;
        }
        if self.end < self.start {
            self.end = self.start;
        }
        self.working_s = (self.end - self.start) as f64;
        self.act_s = [0.0, 0.0, 0.0, self.working_s];
    }

    pub fn hours(&self) -> f64 {
        self.working_s / 3600.0
    }

    /// Its local day, counted from 1970-01-01.
    pub fn day(&self) -> i64 {
        local_day(self.start, self.tz_min)
    }
}

/// What the app tells the book each frame.
#[derive(Clone, Copy, Debug)]
pub struct Moment<'a> {
    /// Unix seconds, with fractions.
    pub now: f64,
    pub tz_min: i32,
    /// CreaBIM's window is in front.
    pub focused: bool,
    /// Seconds since the last mouse move, click or key in CreaBIM.
    pub input_age: f64,
    pub activity: Activity,
    pub person: &'a str,
}

/// What the clock is doing now, for the pill and its dot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClockState {
    /// Tracking is off for this project.
    #[default]
    Off,
    /// Counting working time: the dot beats.
    Counting,
    /// In front but no mouse or key for the idle time: nothing counts.
    Idle,
    /// Another program in front: open time counts, for a while.
    Away,
    /// Paused by hand (or Manual tracking not started).
    Paused,
}

/// The clock's state for this run of the app (not saved).
#[derive(Clone, Debug, Default)]
pub struct Clock {
    pub state: ClockState,
    /// Paused by hand: Pause in the pill's panel, or Manual not started.
    pub paused: bool,
    /// Manual tracking was started this run.
    pub started: bool,
    last: Option<f64>,
    /// When another program came in front.
    away_since: Option<f64>,
    /// The entry being counted, by its index.
    pub open: Option<usize>,
}

impl Clock {
    /// Stop counting into the entry open now (it is kept).
    pub fn close(&mut self) {
        self.open = None;
    }
}

/// Another program in front counts as open time for this long, then
/// nothing counts until CreaBIM is used again.
pub const AWAY_CAP_S: f64 = 30.0 * 60.0;
/// A gap between frames longer than this is a sleep or a hang, not work.
const GAP_S: f64 = 10.0;

/// A project's time and fees, as kept beside it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TimeBook {
    pub version: u32,
    pub tracking: Tracking,
    /// The dot in the pill beats while time counts (REV02).
    pub pulse: bool,
    /// Minutes without mouse or key before the count stops.
    pub idle_min: u32,
    pub scheme: Scheme,
    pub stages: Vec<Stage>,
    /// The stage time goes to now.
    pub stage_now: usize,
    /// Hourly rate, in `currency`.
    pub rate: f64,
    pub currency: String,
    /// Timesheets round each entry to this many minutes (0: none).
    pub rounding_min: u32,
    pub entries: Vec<Entry>,
}

impl Default for TimeBook {
    fn default() -> Self {
        TimeBook::new(Scheme::Riba)
    }
}

impl TimeBook {
    pub fn new(scheme: Scheme) -> TimeBook {
        TimeBook {
            version: 1,
            tracking: Tracking::Off,
            pulse: true,
            idle_min: 5,
            scheme,
            stages: scheme.stages(),
            stage_now: 0,
            rate: 0.0,
            currency: if scheme == Scheme::Aia { "$" } else { "£" }.to_string(),
            rounding_min: 15,
            entries: vec![],
        }
    }

    pub fn stage(&self) -> Option<&Stage> {
        self.stages.get(self.stage_now)
    }

    /// Change the stage list; the stage now and the entries keep their
    /// places where the new list has them.
    pub fn set_scheme(&mut self, scheme: Scheme) {
        if scheme == self.scheme {
            return;
        }
        let used = self.entries.iter().map(|e| e.stage + 1).max().unwrap_or(0);
        let mut stages = scheme.stages();
        // the entries' stages keep a name to show
        while stages.len() < used {
            stages.push(self.stages.get(stages.len()).cloned().unwrap_or_default());
        }
        self.scheme = scheme;
        self.stages = stages;
        self.stage_now = self.stage_now.min(self.stages.len().saturating_sub(1));
    }

    /// Count this frame. True when the book changed (worth saving soon).
    pub fn tick(&mut self, clock: &mut Clock, m: &Moment) -> bool {
        let dt = clock.last.map_or(0.0, |t| m.now - t);
        clock.last = Some(m.now);
        if self.tracking == Tracking::Off || self.stages.is_empty() {
            clock.state = ClockState::Off;
            clock.close();
            return false;
        }
        let paused = clock.paused || (self.tracking == Tracking::Manual && !clock.started);
        if paused {
            clock.state = ClockState::Paused;
            clock.close();
            return false;
        }
        // a sleep or a hang between frames counts as nothing, and ends
        // the stretch
        let dt = if !(0.0..=GAP_S).contains(&dt) {
            clock.close();
            0.0
        } else {
            dt
        };
        let idle_s = f64::from(self.idle_min.max(1)) * 60.0;
        if m.focused {
            clock.away_since = None;
        } else if clock.away_since.is_none() {
            clock.away_since = Some(m.now);
        }
        let state = if !m.focused {
            ClockState::Away
        } else if m.input_age >= idle_s {
            ClockState::Idle
        } else {
            ClockState::Counting
        };
        clock.state = state;
        match state {
            ClockState::Counting => {
                let i = self.entry_for(clock, m);
                let e = &mut self.entries[i];
                e.working_s += dt;
                e.act_s[m.activity.index()] += dt;
                e.end = m.now as i64;
                dt > 0.0
            }
            ClockState::Idle => {
                if let Some(i) = clock.open {
                    if let Some(e) = self.entries.get_mut(i) {
                        e.idle_s += dt;
                    }
                }
                false
            }
            ClockState::Away => {
                let away = clock.away_since.map_or(0.0, |t| m.now - t);
                if away > AWAY_CAP_S {
                    clock.close();
                    return false;
                }
                // open time joins the stretch it interrupts
                let Some(i) = clock.open else { return false };
                match self.entries.get_mut(i) {
                    Some(e) => {
                        e.open_s += dt;
                        dt > 0.0
                    }
                    None => false,
                }
            }
            _ => false,
        }
    }

    /// The entry working time goes into now: the open one while it is the
    /// same stage, person and local day, else a new one.
    fn entry_for(&mut self, clock: &mut Clock, m: &Moment) -> usize {
        let now = m.now as i64;
        if let Some(i) = clock.open {
            if let Some(e) = self.entries.get(i) {
                if !e.manual
                    && e.stage == self.stage_now
                    && e.person == m.person
                    && e.day() == local_day(now, m.tz_min)
                {
                    return i;
                }
            }
        }
        self.entries.push(Entry {
            start: now,
            end: now,
            tz_min: m.tz_min,
            stage: self.stage_now,
            person: m.person.to_string(),
            ..Entry::default()
        });
        clock.open = Some(self.entries.len() - 1);
        self.entries.len() - 1
    }

    /// Hours spent on stage `i`. Added from +0: Rust's `sum` of no f64s
    /// is -0.0, which a stage not started showed as "-0 % used" (owner,
    /// 1.2 film review 2026-11-22).
    pub fn stage_hours(&self, i: usize) -> f64 {
        self.entries
            .iter()
            .filter(|e| e.stage == i)
            .map(Entry::hours)
            .fold(0.0, |a, h| a + h)
    }

    pub fn total_hours(&self) -> f64 {
        self.entries
            .iter()
            .map(Entry::hours)
            .fold(0.0, |a, h| a + h)
    }

    /// How much of stage `i`'s budget is spent (1.0 is all of it); none
    /// without a budget.
    pub fn stage_use(&self, i: usize) -> Option<f64> {
        let b = self.stages.get(i)?.budget_h;
        (b > 0.0).then(|| self.stage_hours(i) / b)
    }

    /// Today's working, open and idle seconds, for `person` or everyone.
    pub fn today(&self, now: i64, tz_min: i32) -> [f64; 3] {
        let day = local_day(now, tz_min);
        self.entries
            .iter()
            .filter(|e| e.day() == day)
            .fold([0.0; 3], |a, e| {
                [a[0] + e.working_s, a[1] + e.open_s, a[2] + e.idle_s]
            })
    }

    /// This week's hours, Monday first, by stage: `[day][stage]`.
    pub fn week(&self, now: i64, tz_min: i32) -> Vec<Vec<f64>> {
        let monday = week_start(local_day(now, tz_min));
        let mut out = vec![vec![0.0; self.stages.len()]; 7];
        for e in &self.entries {
            let d = e.day() - monday;
            if (0..7).contains(&d) {
                if let Some(v) = out[d as usize].get_mut(e.stage) {
                    *v += e.hours();
                }
            }
        }
        out
    }

    /// The hours an entry shows on a timesheet, rounded as set.
    pub fn billed_hours(&self, e: &Entry) -> f64 {
        round_hours(e.hours(), self.rounding_min)
    }

    fn money(&self, v: f64) -> String {
        money(&self.currency, v)
    }

    fn stage_name(&self, i: usize) -> &str {
        self.stages.get(i).map_or("—", |s| s.name.as_str())
    }

    /// The timesheet as CSV, newest last.
    pub fn csv(&self) -> String {
        let mut rows: Vec<&Entry> = self.entries.iter().filter(|e| e.working_s > 0.0).collect();
        rows.sort_by_key(|e| e.start);
        let mut out = String::from(
            "Date,Start,End,Person,Stage,Hours,Modelling,Drawing,Sheets,Other,Value,Note,Typed in\n",
        );
        for e in rows {
            let h = self.billed_hours(e);
            let split: Vec<String> = e
                .act_s
                .iter()
                .map(|s| format!("{:.2}", s / 3600.0))
                .collect();
            out.push_str(&format!(
                "{},{},{},{},{},{:.2},{},{:.2},{},{}\n",
                date_iso(e.start, e.tz_min),
                clock_hm(e.start, e.tz_min),
                clock_hm(e.end, e.tz_min),
                csv_field(&e.person),
                csv_field(self.stage_name(e.stage)),
                h,
                split.join(","),
                h * self.rate,
                csv_field(&e.note),
                if e.manual { "yes" } else { "no" },
            ));
        }
        out
    }

    /// The timesheet as a PDF: an A4 page or more of entries, then each
    /// stage's hours, budget and fee, and the totals.
    pub fn pdf(&self, project: &str, client: &str) -> Vec<u8> {
        use creabim_geom::paper::{Anchor, Canvas, PdfWriter, Stroke};
        use glam::DVec2;
        let size = DVec2::new(210.0, 297.0);
        let mut rows: Vec<&Entry> = self.entries.iter().filter(|e| e.working_s > 0.0).collect();
        rows.sort_by_key(|e| e.start);
        let mut pdf = PdfWriter::new();
        let left = 18.0;
        let right = 192.0;
        let thin = Stroke::solid(0.15, 0.6);
        let heavy = Stroke::solid(0.35, 0.0);
        // columns: date, times, person, stage, note, hours
        let cols = [left, 42.0, 66.0, 88.0, 128.0, right];
        let mut lines: Vec<[String; 6]> = rows
            .iter()
            .map(|e| {
                [
                    date_short(e.start, e.tz_min),
                    format!(
                        "{}–{}",
                        clock_hm(e.start, e.tz_min),
                        clock_hm(e.end, e.tz_min)
                    ),
                    clip(&e.person, 12),
                    clip(self.stage_name(e.stage), 22),
                    clip(&e.note, 34),
                    format!("{:.2}", self.billed_hours(e)),
                ]
            })
            .collect();
        if lines.is_empty() {
            lines.push([
                "".into(),
                "".into(),
                "".into(),
                "No time yet.".into(),
                "".into(),
                "".into(),
            ]);
        }
        let per_page = 44;
        let pages = lines.chunks(per_page).count().max(1);
        let total: f64 = rows
            .iter()
            .map(|e| self.billed_hours(e))
            .fold(0.0, |a, h| a + h);
        for (n, chunk) in lines.chunks(per_page).enumerate() {
            let mut page = pdf.page(size);
            let mut y = 279.0;
            page.text_angle(DVec2::new(left, y), 6.0, "Timesheet", Anchor::Start, 0.0);
            page.text_angle(
                DVec2::new(right, y),
                3.0,
                &format!("Page {} of {}", n + 1, pages),
                Anchor::End,
                0.0,
            );
            y -= 8.0;
            let mut head = project.to_string();
            if !client.is_empty() {
                head.push_str(&format!(" · {client}"));
            }
            page.text_angle(DVec2::new(left, y), 3.6, &head, Anchor::Start, 0.0);
            y -= 5.0;
            let period = match (rows.first(), rows.last()) {
                (Some(a), Some(b)) => format!(
                    "{} to {}",
                    date_short(a.start, a.tz_min),
                    date_short(b.start, b.tz_min)
                ),
                _ => String::new(),
            };
            let mut sub = period;
            if self.rate > 0.0 {
                sub.push_str(&format!(" · {}/h", self.money(self.rate)));
            }
            if self.rounding_min > 0 {
                sub.push_str(&format!(
                    " · each entry rounded to {} min",
                    self.rounding_min
                ));
            }
            page.text_angle(DVec2::new(left, y), 3.0, &sub, Anchor::Start, 0.0);
            y -= 8.0;
            let heads = ["Date", "Time", "Person", "Stage", "Note", "Hours"];
            for (i, h) in heads.iter().enumerate() {
                let (x, a) = if i == 5 {
                    (cols[5], Anchor::End)
                } else {
                    (cols[i], Anchor::Start)
                };
                page.text_angle(DVec2::new(x, y), 2.8, h, a, 0.0);
            }
            y -= 2.5;
            page.line(&[DVec2::new(left, y), DVec2::new(right, y)], heavy);
            y -= 4.0;
            for r in chunk {
                for (i, t) in r.iter().enumerate() {
                    let (x, a) = if i == 5 {
                        (cols[5], Anchor::End)
                    } else {
                        (cols[i], Anchor::Start)
                    };
                    page.text_angle(DVec2::new(x, y), 2.8, t, a, 0.0);
                }
                y -= 2.4;
                page.line(&[DVec2::new(left, y), DVec2::new(right, y)], thin);
                y -= 3.4;
            }
            // the summary under the last page's entries
            if n + 1 == pages {
                y -= 4.0;
                page.text_angle(DVec2::new(left, y), 3.4, "By stage", Anchor::Start, 0.0);
                y -= 6.0;
                for (i, h) in ["Stage", "Hours", "Budget", "Fee", "Value of time"]
                    .iter()
                    .enumerate()
                {
                    let x = [left, 120.0, 140.0, 165.0, right][i];
                    let a = if i == 0 { Anchor::Start } else { Anchor::End };
                    page.text_angle(DVec2::new(x, y), 2.8, h, a, 0.0);
                }
                y -= 2.5;
                page.line(&[DVec2::new(left, y), DVec2::new(right, y)], heavy);
                y -= 4.0;
                for (i, s) in self.stages.iter().enumerate() {
                    let h = self.stage_hours(i);
                    if h <= 0.0 && s.budget_h <= 0.0 && s.fee <= 0.0 {
                        continue;
                    }
                    let cells = [
                        s.name.clone(),
                        format!("{h:.2}"),
                        if s.budget_h > 0.0 {
                            format!("{:.0}", s.budget_h)
                        } else {
                            "—".into()
                        },
                        if s.fee > 0.0 {
                            self.money(s.fee)
                        } else {
                            "—".into()
                        },
                        self.money(h * self.rate),
                    ];
                    for (k, t) in cells.iter().enumerate() {
                        let x = [left, 120.0, 140.0, 165.0, right][k];
                        let a = if k == 0 { Anchor::Start } else { Anchor::End };
                        page.text_angle(DVec2::new(x, y), 2.8, t, a, 0.0);
                    }
                    y -= 2.4;
                    page.line(&[DVec2::new(left, y), DVec2::new(right, y)], thin);
                    y -= 3.4;
                }
                y -= 3.0;
                page.text_angle(
                    DVec2::new(right, y),
                    3.6,
                    &if self.rate > 0.0 {
                        format!("Total {total:.2} h · {}", self.money(total * self.rate))
                    } else {
                        format!("Total {total:.2} h")
                    },
                    Anchor::End,
                    0.0,
                );
            }
        }
        pdf.finish()
    }
}

/// Where a project's time book is kept: beside it, `<name>.creabim-time`.
pub fn book_path(project: &str) -> Option<std::path::PathBuf> {
    if project.trim().is_empty() {
        return None;
    }
    let p = std::path::Path::new(project);
    let stem = p.file_stem()?.to_string_lossy().to_string();
    Some(p.with_file_name(format!("{stem}.creabim-time")))
}

/// Read a project's time book; none when it has none yet.
pub fn load(project: &str) -> Option<TimeBook> {
    let path = book_path(project)?;
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Write a project's time book beside it. Only once the project itself is
/// on disk, and only when there is something to keep.
pub fn store(project: &str, book: &TimeBook) -> Result<(), String> {
    let Some(path) = book_path(project) else {
        return Ok(());
    };
    if !std::path::Path::new(project).exists() {
        return Ok(());
    }
    if book.tracking == Tracking::Off && book.entries.is_empty() && !path.exists() {
        return Ok(());
    }
    let text = serde_json::to_string_pretty(book).map_err(|e| e.to_string())?;
    // written aside then moved over, so a crash never leaves half a file
    let tmp = path.with_extension("creabim-time.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

// ---- time and dates -----------------------------------------------------

/// Unix seconds now, with fractions.
pub fn now_s() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64())
}

/// The computer's offset from UTC now, in minutes (east positive).
pub fn local_offset_min() -> i32 {
    platform_offset_min().unwrap_or(0)
}

#[cfg(windows)]
fn platform_offset_min() -> Option<i32> {
    #[repr(C)]
    struct SystemTime([u16; 8]);
    #[repr(C)]
    struct TimeZoneInformation {
        bias: i32,
        standard_name: [u16; 32],
        standard_date: SystemTime,
        standard_bias: i32,
        daylight_name: [u16; 32],
        daylight_date: SystemTime,
        daylight_bias: i32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetTimeZoneInformation(tz: *mut TimeZoneInformation) -> u32;
    }
    let mut tz = TimeZoneInformation {
        bias: 0,
        standard_name: [0; 32],
        standard_date: SystemTime([0; 8]),
        standard_bias: 0,
        daylight_name: [0; 32],
        daylight_date: SystemTime([0; 8]),
        daylight_bias: 0,
    };
    // SAFETY: the struct has the layout the call writes into
    let r = unsafe { GetTimeZoneInformation(&mut tz) };
    let bias = match r {
        0xFFFF_FFFF => return None,
        2 => tz.bias + tz.daylight_bias,
        1 => tz.bias + tz.standard_bias,
        _ => tz.bias,
    };
    Some(-bias)
}

#[cfg(unix)]
fn platform_offset_min() -> Option<i32> {
    let t = now_s() as libc::time_t;
    // SAFETY: localtime_r fills the zeroed tm it is given
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::localtime_r(&t, &mut tm) };
    if r.is_null() {
        return None;
    }
    Some((tm.tm_gmtoff / 60) as i32)
}

#[cfg(not(any(windows, unix)))]
fn platform_offset_min() -> Option<i32> {
    None
}

/// Is a window of this program in front? The web panes are child windows
/// and take the keyboard, so the window system is asked rather than egui
/// (which counts a click in the toolbar as leaving). None where it can't
/// be told.
#[cfg(windows)]
pub fn app_in_front() -> Option<bool> {
    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> isize;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    }
    // SAFETY: plain queries of the window system
    unsafe {
        let w = GetForegroundWindow();
        if w == 0 {
            return Some(false);
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(w, &mut pid);
        Some(pid == std::process::id())
    }
}

#[cfg(not(windows))]
pub fn app_in_front() -> Option<bool> {
    None
}

/// Seconds since the last mouse move or key anywhere (the web panes
/// included). None where it can't be told.
#[cfg(windows)]
pub fn input_age_s() -> Option<f64> {
    #[repr(C)]
    struct LastInputInfo {
        size: u32,
        time: u32,
    }
    #[link(name = "user32")]
    extern "system" {
        fn GetLastInputInfo(info: *mut LastInputInfo) -> i32;
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetTickCount() -> u32;
    }
    let mut info = LastInputInfo { size: 8, time: 0 };
    // SAFETY: the struct is sized as the call expects
    unsafe {
        if GetLastInputInfo(&mut info) == 0 {
            return None;
        }
        Some(f64::from(GetTickCount().wrapping_sub(info.time)) / 1000.0)
    }
}

#[cfg(not(windows))]
pub fn input_age_s() -> Option<f64> {
    None
}

/// A local day, counted from 1970-01-01.
pub fn local_day(t: i64, tz_min: i32) -> i64 {
    (t + i64::from(tz_min) * 60).div_euclid(86_400)
}

/// The Monday of a day's week.
pub fn week_start(day: i64) -> i64 {
    // 1970-01-01 was a Thursday: Monday is 3 days before
    day - (day + 3).rem_euclid(7)
}

/// A day counted from 1970-01-01 as (year, month, day): Howard Hinnant's
/// civil calendar.
pub fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The days of a civil date counted from 1970-01-01.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
pub const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/// `2026-11-18`.
pub fn date_iso(t: i64, tz_min: i32) -> String {
    let (y, m, d) = civil(local_day(t, tz_min));
    format!("{y:04}-{m:02}-{d:02}")
}

/// `18 Nov 2026`.
pub fn date_short(t: i64, tz_min: i32) -> String {
    let (y, m, d) = civil(local_day(t, tz_min));
    format!("{d} {} {y}", MONTHS[(m - 1) as usize])
}

/// `Wed 18 Nov`.
pub fn day_label(day: i64) -> String {
    let (_, m, d) = civil(day);
    let wd = WEEKDAYS[(day - week_start(day)) as usize];
    format!("{wd} {d} {}", MONTHS[(m - 1) as usize])
}

/// `09:12`, local.
pub fn clock_hm(t: i64, tz_min: i32) -> String {
    let s = (t + i64::from(tz_min) * 60).rem_euclid(86_400);
    format!("{:02}:{:02}", s / 3600, s % 3600 / 60)
}

/// Read `09:12` back as seconds into the day.
pub fn parse_hm(text: &str) -> Option<i64> {
    let (h, m) = text.trim().split_once(':')?;
    let (h, m): (i64, i64) = (h.trim().parse().ok()?, m.trim().parse().ok()?);
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(h * 3600 + m * 60)
}

/// Read `2026-11-18` back as a day counted from 1970-01-01.
pub fn parse_date(text: &str) -> Option<i64> {
    let mut it = text.trim().split('-');
    let y: i64 = it.next()?.parse().ok()?;
    let m: u32 = it.next()?.parse().ok()?;
    let d: u32 = it.next()?.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| days_from_civil(y, m, d))
}

/// `1h 05` from hours.
pub fn hm(hours: f64) -> String {
    let mins = (hours * 60.0).round().max(0.0) as i64;
    format!("{}h {:02}", mins / 60, mins % 60)
}

/// `01:35:26` from seconds.
pub fn clock(secs: f64) -> String {
    let s = secs.max(0.0) as i64;
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

/// Hours rounded to the nearest `step` minutes (none when 0).
pub fn round_hours(h: f64, step: u32) -> f64 {
    if step == 0 {
        return h;
    }
    let step = f64::from(step) / 60.0;
    (h / step).round() * step
}

/// `£1,200` (no pence when whole).
pub fn money(currency: &str, v: f64) -> String {
    let neg = v < 0.0;
    let whole = v.abs().round() as i64;
    let digits = whole.to_string();
    let mut grouped = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{}{currency}{grouped}", if neg { "-" } else { "" })
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(n.saturating_sub(1)).collect();
        t.push('…');
        t
    }
}

/// The bar's state for a stage's use: green, amber from 80 %, red past
/// 100 % (REV02's pulse takes the same colours).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Budget {
    Fine,
    Near,
    Over,
}

pub fn budget_state(used: Option<f64>) -> Budget {
    match used {
        Some(u) if u > 1.0 => Budget::Over,
        Some(u) if u >= 0.8 => Budget::Near,
        _ => Budget::Fine,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(now: f64, focused: bool, input_age: f64) -> Moment<'static> {
        Moment {
            now,
            tz_min: 0,
            focused,
            input_age,
            activity: Activity::Modelling,
            person: "Regis",
        }
    }

    /// Run frames a second apart from `from` for `secs`.
    fn run(
        book: &mut TimeBook,
        clock: &mut Clock,
        from: f64,
        secs: usize,
        focused: bool,
        input_age: f64,
    ) {
        for k in 0..=secs {
            book.tick(clock, &at(from + k as f64, focused, input_age));
        }
    }

    #[test]
    fn a_stage_not_started_has_no_hours_not_minus_zero() {
        // a stage with nothing booked read "-0 % used": the sum of no
        // hours was -0.0
        let b = TimeBook::new(Scheme::Riba);
        assert!(b.entries.is_empty());
        for i in 0..b.stages.len() {
            let h = b.stage_hours(i);
            assert!(h == 0.0 && h.is_sign_positive(), "stage {i}: {h}");
            if let Some(u) = b.stage_use(i) {
                assert_eq!(format!("{:.0} % used", u * 100.0), "0 % used");
            }
        }
        assert!(b.total_hours().is_sign_positive());
    }

    #[test]
    fn off_counts_nothing_and_auto_counts_working_time() {
        let mut b = TimeBook::new(Scheme::Riba);
        let mut c = Clock::default();
        run(&mut b, &mut c, 1_000_000.0, 60, true, 0.0);
        assert!(b.entries.is_empty());
        assert_eq!(c.state, ClockState::Off);
        b.tracking = Tracking::Auto;
        run(&mut b, &mut c, 1_000_100.0, 60, true, 0.0);
        assert_eq!(b.entries.len(), 1);
        assert!((b.entries[0].working_s - 60.0).abs() < 1e-6);
        assert!((b.entries[0].act_s[0] - 60.0).abs() < 1e-6);
        assert_eq!(c.state, ClockState::Counting);
    }

    #[test]
    fn idle_stops_the_count_and_another_program_counts_open_time() {
        let mut b = TimeBook::new(Scheme::Riba);
        b.tracking = Tracking::Auto;
        let mut c = Clock::default();
        run(&mut b, &mut c, 0.0, 10, true, 0.0);
        run(&mut b, &mut c, 11.0, 10, true, 400.0);
        assert_eq!(c.state, ClockState::Idle);
        assert!((b.entries[0].working_s - 10.0).abs() < 1e-6);
        assert!(b.entries[0].idle_s > 9.0);
        run(&mut b, &mut c, 22.0, 20, false, 0.0);
        assert_eq!(c.state, ClockState::Away);
        assert!(b.entries[0].open_s > 19.0);
        assert!((b.entries[0].working_s - 10.0).abs() < 1e-6);
    }

    #[test]
    fn a_stage_change_or_a_pause_starts_a_new_entry() {
        let mut b = TimeBook::new(Scheme::Riba);
        b.tracking = Tracking::Auto;
        let mut c = Clock::default();
        run(&mut b, &mut c, 0.0, 5, true, 0.0);
        b.stage_now = 2;
        run(&mut b, &mut c, 6.0, 5, true, 0.0);
        assert_eq!(b.entries.len(), 2);
        assert_eq!(b.entries[1].stage, 2);
        c.paused = true;
        run(&mut b, &mut c, 12.0, 5, true, 0.0);
        assert_eq!(c.state, ClockState::Paused);
        c.paused = false;
        run(&mut b, &mut c, 18.0, 5, true, 0.0);
        assert_eq!(b.entries.len(), 3);
        // each run counts a second per frame, the first included
        assert!((b.stage_hours(2) * 3600.0 - 12.0).abs() < 1e-6);
    }

    #[test]
    fn manual_tracking_waits_for_start_and_a_sleep_counts_nothing() {
        let mut b = TimeBook::new(Scheme::Riba);
        b.tracking = Tracking::Manual;
        let mut c = Clock::default();
        run(&mut b, &mut c, 0.0, 5, true, 0.0);
        assert!(b.entries.is_empty());
        c.started = true;
        run(&mut b, &mut c, 6.0, 5, true, 0.0);
        // the machine slept for an hour between two frames
        b.tick(&mut c, &at(3_611.0, true, 0.0));
        b.tick(&mut c, &at(3_612.0, true, 0.0));
        // the hour asleep is not counted: six seconds, then the one after
        let total: f64 = b.entries.iter().map(|e| e.working_s).sum();
        assert!((total - 7.0).abs() < 1e-6, "{total}");
        assert_eq!(b.entries.len(), 2);
    }

    #[test]
    fn days_weeks_and_clock_text_are_local() {
        // 2026-11-18 is a Wednesday
        let day = days_from_civil(2026, 11, 18);
        assert_eq!(civil(day), (2026, 11, 18));
        assert_eq!(day_label(day), "Wed 18 Nov");
        assert_eq!(day_label(week_start(day)), "Mon 16 Nov");
        // 23:30 UTC is the next day an hour east
        let t = day * 86_400 + 23 * 3600 + 30 * 60;
        assert_eq!(date_iso(t, 0), "2026-11-18");
        assert_eq!(date_iso(t, 60), "2026-11-19");
        assert_eq!(clock_hm(t, 60), "00:30");
        assert_eq!(parse_hm("9:05"), Some(9 * 3600 + 5 * 60));
        assert_eq!(parse_date("2026-11-18"), Some(day));
        assert_eq!(hm(1.6), "1h 36");
        assert_eq!(clock(5726.0), "01:35:26");
    }

    #[test]
    fn timesheets_round_and_carry_every_entry() {
        let mut b = TimeBook::new(Scheme::Riba);
        b.rate = 85.0;
        b.rounding_min = 15;
        let mut e = Entry::manual(1_700_000_000, 0, 2, "Regis");
        e.end = e.start + 50 * 60;
        e.note = "Site visit, with \"client\"".into();
        e.fit_manual();
        b.entries.push(e);
        assert!((b.billed_hours(&b.entries[0]) - 0.75).abs() < 1e-9);
        let csv = b.csv();
        assert!(csv.lines().count() == 2, "{csv}");
        assert!(csv.contains("\"Site visit, with \"\"client\"\"\""), "{csv}");
        assert!(csv.contains(",0.75,"), "{csv}");
        let pdf = b.pdf("Maple Lane House", "The Rivera family");
        assert!(pdf.starts_with(b"%PDF"));
        assert_eq!(money("£", 4200.0), "£4,200");
    }

    #[test]
    fn the_book_lives_beside_the_project_and_round_trips() {
        let p = book_path("C:/jobs/Maple Lane.creabim").unwrap();
        assert!(p.to_string_lossy().ends_with("Maple Lane.creabim-time"));
        let mut b = TimeBook::new(Scheme::Aia);
        b.tracking = Tracking::Auto;
        b.entries.push(Entry::manual(10, 0, 1, "Ana"));
        let text = serde_json::to_string(&b).unwrap();
        let back: TimeBook = serde_json::from_str(&text).unwrap();
        assert_eq!(back, b);
        // a book from an older or newer CreaBIM still reads
        let thin: TimeBook = serde_json::from_str(r#"{"tracking":"Auto"}"#).unwrap();
        assert_eq!(thin.tracking, Tracking::Auto);
        assert_eq!(thin.idle_min, 5);
    }

    #[test]
    fn budgets_go_amber_at_80_and_red_past_100() {
        assert_eq!(budget_state(None), Budget::Fine);
        assert_eq!(budget_state(Some(0.79)), Budget::Fine);
        assert_eq!(budget_state(Some(0.87)), Budget::Near);
        assert_eq!(budget_state(Some(1.01)), Budget::Over);
        assert_eq!(Stage::new("2 Concept design").short(), "Concept design");
    }
}

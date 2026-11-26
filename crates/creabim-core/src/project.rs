//! The project's own information: what it is, where, and for whom. One
//! element at most holds it; every field is optional. It fills the
//! sheets' title blocks (under any value a sheet sets itself) and names
//! the project, site and building in IFC exports (A7-5).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProjectInfo {
    /// The project's name ("Maple Lane House").
    pub name: String,
    /// The office's job number.
    pub number: String,
    pub client: String,
    /// "Planning application", "Tender", "Construction".
    pub status: String,
    /// The issue date, as the office writes it.
    pub issue_date: String,
    /// The site's postal address.
    pub address: String,
    pub site_name: String,
    /// Degrees, north positive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    /// Degrees, east positive.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    /// The project's zero above sea level, metres.
    pub elevation: f64,
    /// How levels on the site are read and typed in the app: from the
    /// datum (100.45) or from the project's zero (+0.45). One height is
    /// kept either way.
    #[serde(skip_serializing_if = "LevelReading::is_datum")]
    pub site_levels: LevelReading,
    /// How far true north turns from the drawings' up, degrees anticlockwise.
    pub true_north: f64,
    pub building_name: String,
    /// "Dwelling", "Office", "School".
    pub building_use: String,
    /// The project's notepad (MU26): notes and to-dos, markdown, kept with
    /// the project and never printed. Read into [`ProjectInfo::pad`] once.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// The notepad's items (MU27): to-dos and notes, each with its text
    /// (the first line its title) and, if wanted, a link to an element or
    /// a view.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pad: Vec<PadItem>,
    /// The drawing notes (Properties REV04, decision 9): General,
    /// Structural, Electrical, Plumbing and the user's own, each Markdown,
    /// any of them shown on sheets by a linked note. Empty is the four
    /// starting sections, blank. Drafts and To-dos are [`ProjectInfo::pad`].
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<NoteSection>,
    /// The month 3D shows the project's trees in, 1 (January) to 12; 0
    /// is July. A tree can keep a month of its own.
    #[serde(skip_serializing_if = "is_zero")]
    pub month: u8,
    /// Where the project's origin is on the map; none is not placed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<MapPlace>,
    /// The project's own profiles (MU27 block 5), for sweeps.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<crate::profiles::NamedProfile>,
    /// The project's own stair rule sets, edited from the presets.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stair_rules: Vec<crate::stairrules::StairRules>,
    /// The project's phases in order (see [`crate::phases`]); empty is a
    /// project that is not phased.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<crate::phases::PhaseDef>,
    /// How each phase status draws: the colour of what comes down, and so
    /// on.
    #[serde(skip_serializing_if = "crate::phases::PhaseLook::is_default")]
    pub phase_look: crate::phases::PhaseLook,
    /// The questions the design has open, each with its options.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub questions: Vec<crate::phases::Question>,
    /// Questions settled, kept so they can be opened again with what
    /// their other options still hold.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub shelf: Vec<crate::phases::Question>,
    /// Groups of elements, as IFC keeps them (see [`crate::groups`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<crate::groups::GroupDef>,
    /// Copies made so a piece of several options' groups is in each
    /// option (see [`crate::groups::make_options`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub option_copies: Vec<crate::groups::OptionCopy>,
    /// How the project draws and makes its doors and windows.
    #[serde(skip_serializing_if = "OpeningConventions::is_default")]
    pub openings: OpeningConventions,
    /// How far the ground keeps from what is built (MU140): the room under
    /// a framed floor, and the gap between the ground and the wood.
    #[serde(skip_serializing_if = "GroundRules::is_default")]
    pub ground: GroundRules,
}

/// How far the ground keeps from what is built, metres: the usual rules
/// for timber on the ground (the US code's 18 in, 12 in and 8 in), the
/// project's to change for another country's.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GroundRules {
    /// The room under a framed floor's joists: a crawl space's depth.
    pub crawl: f64,
    /// The room under a beam in a crawl space.
    pub crawl_beam: f64,
    /// How far the ground round the house stays below the wood.
    pub wood: f64,
}

impl Default for GroundRules {
    fn default() -> Self {
        Self {
            crawl: 0.4572,
            crawl_beam: 0.3048,
            wood: 0.2032,
        }
    }
}

impl GroundRules {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// The project's rules, or the usual ones.
    pub fn of(model: &crate::Model) -> Self {
        model
            .project_info()
            .map(|(_, p)| p.ground)
            .unwrap_or_default()
    }
}

/// The project's conventions for doors and windows: how elevations mark
/// an opening panel, and which way new casements open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct OpeningConventions {
    /// Where an opening panel's triangle in elevation points.
    pub triangle_apex: TriangleApex,
    /// Which way a new casement opens; each can still be flipped.
    pub casements: CasementSide,
}

impl OpeningConventions {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// Where an opening panel's triangle in elevation points: to its hinges
/// (as in North America and the UK) or to its handle (as in much of
/// Europe).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TriangleApex {
    #[default]
    Hinge,
    Handle,
}

/// Which way new casements open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CasementSide {
    #[default]
    Outward,
    Inward,
}

/// How a level on the site is read and typed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelReading {
    /// Above the project's datum, as site drawings write it: 100.45.
    #[default]
    Datum,
    /// Above the project's zero, as building levels are: +0.45.
    Zero,
}

impl LevelReading {
    pub fn is_datum(&self) -> bool {
        *self == LevelReading::Datum
    }
}

/// Where the project sits on the map: its origin's coordinates in a map
/// coordinate system, metres. The drawings' up is the map's grid north.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MapPlace {
    /// The coordinate system as its users write it: an EPSG code
    /// ("EPSG:32617") or a name ("NAD83 / UTM zone 17N").
    #[serde(skip_serializing_if = "String::is_empty")]
    pub crs: String,
    pub easting: f64,
    pub northing: f64,
}

impl MapPlace {
    /// The EPSG number of its coordinate system, when it is written as one.
    pub fn epsg(&self) -> Option<u32> {
        let t = self.crs.trim();
        let t = t
            .strip_prefix("EPSG:")
            .or_else(|| t.strip_prefix("epsg:"))
            .unwrap_or(t);
        t.trim().parse().ok()
    }

    /// A point of the model on the map.
    pub fn to_map(&self, p: glam::DVec2) -> glam::DVec2 {
        p + glam::DVec2::new(self.easting, self.northing)
    }

    /// A point of the map in the model.
    pub fn to_model(&self, p: glam::DVec2) -> glam::DVec2 {
        p - glam::DVec2::new(self.easting, self.northing)
    }
}

fn is_zero(v: &u8) -> bool {
    *v == 0
}

impl ProjectInfo {
    /// The month its trees show in 3D, 1 to 12.
    pub fn season(&self) -> u8 {
        if (1..=12).contains(&self.month) {
            self.month
        } else {
            7
        }
    }
}

/// The section a sheet note shows for the project's drafts: its notepad
/// items that are not to-dos.
pub const DRAFTS: &str = "drafts";
/// The section of the project's to-dos.
pub const TODOS: &str = "todos";

/// One section of the project's notes: a heading of its own in the Notes
/// area, its text Markdown (headings, lists, checkboxes) with `{fields}`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NoteSection {
    /// What sheet notes link to; never changes, whatever its name.
    pub key: String,
    pub name: String,
    pub text: String,
}

impl NoteSection {
    /// A section headed with its name as typed, in capitals: nothing is
    /// added to it (MU88: "Landscape" heads LANDSCAPE).
    pub fn new(key: &str, name: &str) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            text: format!(
                "## {}

",
                name.trim().to_uppercase()
            ),
        }
    }
}

/// The sections a project starts with.
pub fn starting_sections() -> Vec<NoteSection> {
    vec![
        NoteSection::new("general", "General notes"),
        NoteSection::new("structural", "Structural notes"),
        NoteSection::new("electrical", "Electrical notes"),
        NoteSection::new("plumbing", "Plumbing notes"),
    ]
}

impl ProjectInfo {
    /// Its drawing-note sections: its own, else the four it starts with.
    pub fn note_sections(&self) -> Vec<NoteSection> {
        if self.sections.is_empty() {
            starting_sections()
        } else {
            self.sections.clone()
        }
    }

    /// The name of a section, by its key ("General", "Drafts").
    pub fn section_name(&self, key: &str) -> Option<String> {
        match key {
            DRAFTS => Some("Drafts".into()),
            TODOS => Some("To-dos".into()),
            k => self
                .note_sections()
                .into_iter()
                .find(|s| s.key == k)
                .map(|s| s.name),
        }
    }

    /// A section's text as Markdown, as a sheet note shows it: drafts as a
    /// list, to-dos with their boxes. None when it is gone.
    pub fn section_markdown(&self, key: &str) -> Option<String> {
        let pad = self.pad_items();
        match key {
            DRAFTS => {
                let mut out = String::from("## DRAFTS\n\n");
                for i in pad.iter().filter(|i| !i.todo) {
                    let mut lines = i.text.lines();
                    out.push_str(&format!("- {}\n", lines.next().unwrap_or("").trim()));
                    for l in lines {
                        out.push_str(&format!("  {}\n", l.trim()));
                    }
                }
                Some(out)
            }
            TODOS => {
                let mut out = String::from("## TO-DOS\n\n");
                for i in pad.iter().filter(|i| i.todo) {
                    out.push_str(&format!(
                        "- [{}] {}\n",
                        if i.done { "x" } else { " " },
                        i.title()
                    ));
                }
                Some(out)
            }
            k => self
                .note_sections()
                .into_iter()
                .find(|s| s.key == k)
                .map(|s| s.text),
        }
    }

    /// The notepad's items, the old markdown notepad read as items when it
    /// has none.
    pub fn pad_items(&self) -> Vec<PadItem> {
        if self.pad.is_empty() && !self.notes.trim().is_empty() {
            pad_from_markdown(&self.notes)
        } else {
            self.pad.clone()
        }
    }

    /// A key for a new section of the user's own.
    pub fn new_section_key(&self) -> String {
        let have = self.note_sections();
        (1..)
            .map(|n| format!("own-{n}"))
            .find(|k| !have.iter().any(|s| &s.key == k))
            .unwrap_or_default()
    }
}

/// The values `{fields}` in notes take: the project's information, its
/// zero's elevation and its sheets' numbers.
pub fn note_fields(model: &crate::Model) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    if let Some((_, p)) = model.project_info() {
        for (k, v) in p.fields() {
            out.insert(k.to_string(), v);
        }
        out.insert("elevation".into(), format!("{:+.3} m", p.elevation));
    }
    let mut numbers: Vec<String> = model
        .iter()
        .filter_map(|e| match &e.kind {
            crate::ElementKind::Sheet(s) if !s.number.trim().is_empty() => {
                Some(s.number.trim().to_string())
            }
            _ => None,
        })
        .collect();
    numbers.sort();
    if !numbers.is_empty() {
        out.insert("sheets".into(), numbers.join(", "));
    }
    out.insert("date".into(), today());
    out
}

/// Today's date as YYYY-MM-DD (UTC), for the {date} field.
pub fn today() -> String {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 86_400) as i64;
    // days since 1970-01-01 to a civil date (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The fields a note can carry, with what each fills in.
pub const NOTE_FIELDS: [(&str, &str); 9] = [
    ("project", "The project's name"),
    ("project_number", "The job number"),
    ("client", "The client"),
    ("address", "The site's address"),
    ("status", "The project's status"),
    ("issue_date", "The issue date"),
    ("elevation", "Project zero above datum"),
    ("sheets", "Every sheet's number"),
    ("date", "Today's date, as the sheet is printed"),
];

/// The text a note linked to the section `key` shows, its fields filled.
pub fn linked_text(model: &crate::Model, key: &str) -> Option<String> {
    let info = model
        .project_info()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();
    let md = info.section_markdown(key)?;
    Some(crate::titleblock::fill_fields(&md, &note_fields(model)))
}

/// One item of the project's notepad.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PadItem {
    pub text: String,
    /// A to-do (ticked or not); else a note.
    pub todo: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub done: bool,
    /// What it is about: an element or a view, selected or opened from it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<crate::ElementId>,
}

impl PadItem {
    /// Its first line.
    pub fn title(&self) -> &str {
        self.text.lines().next().unwrap_or("").trim()
    }
}

/// The old markdown notepad as items: `- [ ]` and `- [x]` lines are
/// to-dos, other lines join into notes, a blank line or heading ending one.
pub fn pad_from_markdown(md: &str) -> Vec<PadItem> {
    let mut out: Vec<PadItem> = vec![];
    let mut note: Option<PadItem> = None;
    let flush = |note: &mut Option<PadItem>, out: &mut Vec<PadItem>| {
        if let Some(n) = note.take().filter(|n| !n.text.trim().is_empty()) {
            out.push(n);
        }
    };
    for line in md.lines() {
        let t = line.trim();
        let todo = t.strip_prefix("- [ ] ").map(|r| (false, r)).or_else(|| {
            t.strip_prefix("- [x] ")
                .or(t.strip_prefix("- [X] "))
                .map(|r| (true, r))
        });
        if let Some((done, r)) = todo {
            flush(&mut note, &mut out);
            out.push(PadItem {
                text: r.into(),
                todo: true,
                done,
                link: None,
            });
        } else if t.is_empty() {
            flush(&mut note, &mut out);
        } else {
            let t = t.trim_start_matches('#').trim();
            match &mut note {
                Some(n) => {
                    n.text.push('\n');
                    n.text.push_str(t);
                }
                None => {
                    note = Some(PadItem {
                        text: t.into(),
                        ..Default::default()
                    })
                }
            }
        }
    }
    flush(&mut note, &mut out);
    out
}

/// Items as markdown: to-dos as `- [ ]`/`- [x]` lines, notes as
/// paragraphs, each link named after what it points at.
pub fn pad_to_markdown(
    items: &[&PadItem],
    name_of: &dyn Fn(crate::ElementId) -> Option<String>,
) -> String {
    let mut todos = String::new();
    let mut notes = String::new();
    for i in items {
        let link = i
            .link
            .and_then(name_of)
            .map(|n| format!(" (\u{2192} {n})"))
            .unwrap_or_default();
        if i.todo {
            let mut lines = i.text.lines();
            let first = lines.next().unwrap_or("");
            todos.push_str(&format!(
                "- [{}] {first}{link}\n",
                if i.done { "x" } else { " " }
            ));
            for l in lines {
                todos.push_str(&format!("  {l}\n"));
            }
        } else {
            notes.push_str(&format!("{}{link}\n\n", i.text.trim_end()));
        }
    }
    let mut out = String::new();
    if !todos.is_empty() {
        out.push_str("## To-dos\n\n");
        out.push_str(&todos);
        out.push('\n');
    }
    if !notes.is_empty() {
        out.push_str("## Notes\n\n");
        out.push_str(&notes);
    }
    out.trim_end().to_string() + "\n"
}

impl ProjectInfo {
    /// The title-block fields this fills, by the names title blocks use:
    /// only those with a value.
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        [
            ("job", &self.name),
            ("project", &self.name),
            ("project_number", &self.number),
            ("client", &self.client),
            ("status", &self.status),
            ("issue_date", &self.issue_date),
            ("address", &self.address),
            ("site", &self.site_name),
            ("building", &self.building_name),
        ]
        .into_iter()
        .filter(|(_, v)| !v.trim().is_empty())
        .map(|(k, v)| (k, v.trim().to_string()))
        .collect()
    }

    /// Nothing filled in.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// The change that makes `info` the project's information: one undo
/// step, nothing when it is already so.
pub fn set_project(model: &crate::Model, info: ProjectInfo) -> crate::Transaction {
    let tx = crate::Transaction::new("Project information");
    match model.project_info() {
        Some((_, p)) if *p == info => tx,
        Some((id, _)) => {
            let Some(e) = model.get(id) else { return tx };
            let mut after = e.clone();
            after.kind = crate::ElementKind::Project(info);
            tx.replace(e.clone(), after)
        }
        None => tx.insert(crate::Element::new(
            "Project information",
            crate::ElementKind::Project(info),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_filled_fields_reach_the_title_blocks() {
        let p = ProjectInfo {
            name: "Maple Lane House".into(),
            client: "  ".into(),
            ..Default::default()
        };
        let f = p.fields();
        assert!(f.contains(&("job", "Maple Lane House".to_string())));
        assert!(f.contains(&("project", "Maple Lane House".to_string())));
        assert!(!f.iter().any(|(k, _)| *k == "client"));
        assert!(ProjectInfo::default().is_empty());
    }

    #[test]
    fn the_old_notepad_reads_as_items_and_writes_back_as_markdown() {
        let md = "- [x] Start a new project\n- [ ] testing a new note\n\nA note\nits second line";
        let items = pad_from_markdown(md);
        assert_eq!(items.len(), 3);
        assert!(items[0].todo && items[0].done);
        assert_eq!(items[2].title(), "A note");
        let refs: Vec<&PadItem> = items.iter().collect();
        let back = pad_to_markdown(&refs, &|_| None);
        assert!(back.contains("- [x] Start a new project"));
        assert!(back.contains("## Notes"));
        assert_eq!(
            pad_from_markdown(&back).iter().filter(|i| i.todo).count(),
            2
        );
    }

    #[test]
    fn setting_it_twice_keeps_one_element() {
        let mut doc = crate::document::Document::new();
        let a = ProjectInfo {
            name: "A".into(),
            ..Default::default()
        };
        doc.commit(set_project(doc.model(), a.clone())).unwrap();
        let b = ProjectInfo {
            name: "B".into(),
            ..a.clone()
        };
        doc.commit(set_project(doc.model(), b)).unwrap();
        let n = doc
            .model()
            .iter()
            .filter(|e| matches!(e.kind, crate::ElementKind::Project(_)))
            .count();
        assert_eq!(n, 1);
        assert_eq!(doc.model().project_info().unwrap().1.name, "B");
        assert!(
            set_project(doc.model(), doc.model().project_info().unwrap().1.clone())
                .ops
                .is_empty()
        );
    }
}

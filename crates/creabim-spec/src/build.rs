//! A model description into a model. Every id the file uses is checked
//! against the kind of element it must name (a door's wall must be a
//! wall), every element against what the model accepts, and each problem
//! is reported at its path in the file. An element with a problem is left
//! out and what refers to it says nothing more, so one mistake gives one
//! message.

use crate::format as f;
use crate::json::{suggest, Path, Problem, Spec, J};
use creabim_core::{
    self as core, parts, site, types, CatalogueObject, Document, Element, ElementId, ElementKind,
    Model, Transaction,
};
use glam::{DVec2, DVec3};
use std::collections::{HashMap, HashSet};

/// A built model, and what was worth saying about the file on the way.
pub struct Built {
    pub model: Model,
    /// Things that built but look wrong (a window taller than its wall).
    pub warnings: Vec<Problem>,
}

/// What an id names, so a reference to the wrong kind is caught.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tag {
    Material,
    Assembly,
    Type(f::TypeCategory),
    Level,
    GridGroup,
    Grid,
    Wall,
    Opening,
    Slab,
    Roof,
    Column,
    Beam,
    Stair,
    Railing,
    Part,
    Site,
    Object,
    Room,
    View,
    Sheet,
}

impl Tag {
    fn word(self) -> String {
        match self {
            Tag::Material => "a material".into(),
            Tag::Assembly => "an assembly".into(),
            Tag::Type(c) => format!("a {} type", c.key()),
            Tag::Level => "a level".into(),
            Tag::GridGroup => "a grid group".into(),
            Tag::Grid => "a grid".into(),
            Tag::Wall => "a wall".into(),
            Tag::Opening => "an opening".into(),
            Tag::Slab => "a slab".into(),
            Tag::Roof => "a roof".into(),
            Tag::Column => "a column".into(),
            Tag::Beam => "a beam".into(),
            Tag::Stair => "a stair".into(),
            Tag::Railing => "a railing".into(),
            Tag::Part => "a part".into(),
            Tag::Site => "a site element".into(),
            Tag::Object => "an object".into(),
            Tag::Room => "a room".into(),
            Tag::View => "a view".into(),
            Tag::Sheet => "a sheet".into(),
        }
    }
}

struct Ctx {
    /// Metres in one of the file's units.
    k: f64,
    ids: HashMap<String, (ElementId, Tag, Path)>,
    /// Ids whose element had a problem: what refers to them is quiet.
    broken: HashSet<String>,
    errors: Vec<Problem>,
    warnings: Vec<Problem>,
    doc: Document,
}

impl Ctx {
    fn l(&self, v: f64) -> f64 {
        v * self.k
    }
    fn p(&self, v: DVec2) -> DVec2 {
        v * self.k
    }
    fn p3(&self, v: DVec3) -> DVec3 {
        v * self.k
    }
    fn pts(&self, v: &[DVec2]) -> Vec<DVec2> {
        v.iter().map(|p| self.p(*p)).collect()
    }
    fn model(&self) -> &Model {
        self.doc.model()
    }
    fn err(&mut self, at: &Path, msg: impl Into<String>) {
        self.errors.push(Problem::new(at, msg));
    }

    /// Claim `id` for an element at `at`.
    fn claim(&mut self, id: &str, tag: Tag, at: &Path) {
        if id.is_empty() {
            self.err(&at.key("id"), "an id can't be empty");
            return;
        }
        if let Some((_, _, first)) = self.ids.get(id) {
            let first = first.clone();
            self.err(
                &at.key("id"),
                format!("the id \"{id}\" is already used by {first}"),
            );
            return;
        }
        self.ids
            .insert(id.to_string(), (ElementId::new(), tag, at.clone()));
    }

    /// The element `id` was claimed for, if it is one of `want`.
    fn get(&mut self, id: &str, want: &[Tag], at: &Path) -> Option<ElementId> {
        if self.broken.contains(id) {
            return None;
        }
        let words = || {
            want.iter()
                .map(|t| t.word())
                .collect::<Vec<_>>()
                .join(" or ")
        };
        match self.ids.get(id) {
            Some((eid, tag, _)) if want.contains(tag) => Some(*eid),
            Some((_, tag, _)) => {
                let msg = format!("\"{id}\" is {}, not {}", tag.word(), words());
                self.err(at, msg);
                None
            }
            None => {
                let known: Vec<&str> = self
                    .ids
                    .iter()
                    .filter(|(_, (_, t, _))| want.contains(t))
                    .map(|(k, _)| k.as_str())
                    .collect();
                let hint = suggest(id, &known)
                    .map(|s| format!("; did you mean \"{s}\"?"))
                    .unwrap_or_default();
                self.err(
                    at,
                    format!("there is no {} with the id \"{id}\"{hint}", {
                        let w = words();
                        w.split_once(' ').map(|(_, r)| r.to_string()).unwrap_or(w)
                    }),
                );
                None
            }
        }
    }

    fn opt(&mut self, id: &Option<String>, want: &[Tag], at: &Path) -> Option<Option<ElementId>> {
        match id {
            None => Some(None),
            Some(i) => self.get(i, want, at).map(Some),
        }
    }

    fn eid(&self, id: &str) -> ElementId {
        self.ids[id].0
    }

    /// Put `e` (claimed as `id`, described at `at`) into the model.
    /// Did the element at `at` claim `id`? Not when another had it first
    /// (already reported).
    fn owns(&self, id: &str, at: &Path) -> bool {
        self.ids.get(id).is_some_and(|(_, _, p)| p == at)
    }

    fn insert(&mut self, id: &str, mut e: Element, at: &Path) {
        if !self.owns(id, at) {
            return;
        }
        e.id = self.eid(id);
        self.commit(id, Transaction::new("Build").insert(e), at);
    }

    fn commit(&mut self, id: &str, tx: Transaction, at: &Path) {
        for op in &tx.ops {
            if let core::Op::Insert(e) = op {
                if let Err(m) = e.validate() {
                    self.err(at, m);
                    self.broken.insert(id.to_string());
                    return;
                }
            }
        }
        if let Err(e) = self.doc.commit(tx) {
            self.err(at, e.to_string());
            self.broken.insert(id.to_string());
        }
    }

    /// Note that the element `id` had a problem.
    fn fail(&mut self, id: &str) {
        self.broken.insert(id.to_string());
    }
}

/// The keys of `v` as written that `allowed` doesn't list: fields given
/// that its kind doesn't use.
fn stray(v: &impl Spec, allowed: &[&str]) -> Vec<String> {
    match v.write() {
        J::Obj(o) => o
            .into_iter()
            .map(|(k, _)| k)
            .filter(|k| !allowed.contains(&k.as_str()))
            .collect(),
        _ => vec![],
    }
}

fn name_of(name: &Option<String>, id: &str) -> String {
    name.clone().unwrap_or_else(|| id.to_string())
}

/// Turn a model description into a model.
pub fn build(spec: &f::Model) -> Result<Built, Vec<Problem>> {
    let mut c = Ctx {
        k: spec.units.metres(),
        ids: HashMap::new(),
        broken: HashSet::new(),
        errors: vec![],
        warnings: vec![],
        doc: Document::new(),
    };
    let root = Path::root();
    claim_all(&mut c, spec, &root);

    if let Some(u) = &spec.display_units {
        let length = match u.length {
            f::DisplayLength::Millimetres => core::LengthUnit::Millimetres,
            f::DisplayLength::Centimetres => core::LengthUnit::Centimetres,
            f::DisplayLength::Metres => core::LengthUnit::Metres,
            f::DisplayLength::FeetInches => core::LengthUnit::FeetInches,
            f::DisplayLength::Feet => core::LengthUnit::Feet,
            f::DisplayLength::Inches => core::LengthUnit::Inches,
        };
        let units = core::Units {
            length,
            precision: u.precision.min(9) as u8,
        };
        c.commit(
            "",
            Transaction::new("Build").insert(Element::new("Units", ElementKind::Units(units))),
            &root.key("display_units"),
        );
    }

    if let Some(z) = spec.datum.filter(|z| z.is_finite()) {
        let info = core::ProjectInfo {
            elevation: z,
            ..Default::default()
        };
        c.commit(
            "",
            Transaction::new("Build").insert(Element::new("Project", ElementKind::Project(info))),
            &root.key("datum"),
        );
    }

    for (i, m) in spec.materials.iter().enumerate() {
        material(&mut c, m, &root.key("materials").index(i));
    }
    for (i, a) in spec.assemblies.iter().enumerate() {
        assembly(&mut c, a, &root.key("assemblies").index(i));
    }
    for (i, t) in spec.types.iter().enumerate() {
        element_type(&mut c, t, &root.key("types").index(i));
    }
    for (i, l) in spec.levels.iter().enumerate() {
        let at = root.key("levels").index(i);
        let level = core::Level {
            elevation: c.l(l.elevation),
            height: c.l(l.height),
        };
        c.insert(
            &l.id,
            Element::new(name_of(&l.name, &l.id), ElementKind::Level(level)),
            &at,
        );
    }
    for (i, g) in spec.grid_groups.iter().enumerate() {
        let at = root.key("grid_groups").index(i);
        let label = |s: f::GridLabels| match s {
            f::GridLabels::Numbers => core::grids::LabelStyle::Numbers,
            f::GridLabels::Letters => core::grids::LabelStyle::Letters,
        };
        let mut group = core::GridGroup {
            prefix: g.prefix.clone(),
            vertical: label(g.vertical),
            horizontal: label(g.horizontal),
            ..Default::default()
        };
        if let Some(col) = g.colour {
            group.color = col.0;
        }
        c.insert(
            &g.id,
            Element::new(name_of(&g.name, &g.id), ElementKind::GridGroup(group)),
            &at,
        );
    }
    for (i, g) in spec.grids.iter().enumerate() {
        let at = root.key("grids").index(i);
        let Some(group) = c.opt(&g.group, &[Tag::GridGroup], &at.key("group")) else {
            c.fail(&g.id);
            continue;
        };
        let grid = core::Grid {
            label: g.label.clone().unwrap_or_else(|| g.id.clone()),
            start: c.p(g.start),
            end: c.p(g.end),
            group,
            ..Default::default()
        };
        let name = g
            .name
            .clone()
            .unwrap_or_else(|| format!("Grid {}", grid.label));
        c.insert(&g.id, Element::new(name, ElementKind::Grid(grid)), &at);
    }
    for (i, w) in spec.walls.iter().enumerate() {
        wall(&mut c, w, &root.key("walls").index(i));
    }
    for (i, s) in spec.slabs.iter().enumerate() {
        slab(&mut c, s, &root.key("slabs").index(i));
    }
    for (i, r) in spec.roofs.iter().enumerate() {
        roof(&mut c, r, &root.key("roofs").index(i));
    }
    for (i, o) in spec.openings.iter().enumerate() {
        opening(&mut c, o, &root.key("openings").index(i));
    }
    for (i, col) in spec.columns.iter().enumerate() {
        column(&mut c, col, &root.key("columns").index(i));
    }
    for (i, b) in spec.beams.iter().enumerate() {
        beam(&mut c, b, &root.key("beams").index(i));
    }
    for (i, s) in spec.stairs.iter().enumerate() {
        stair(&mut c, s, &root.key("stairs").index(i));
    }
    for (i, r) in spec.railings.iter().enumerate() {
        railing(&mut c, r, &root.key("railings").index(i));
    }
    for (i, p) in spec.parts.iter().enumerate() {
        part(&mut c, p, &root.key("parts").index(i));
    }
    for (i, s) in spec.site.iter().enumerate() {
        site_element(&mut c, s, &root.key("site").index(i));
    }
    let catalogue = core::catalogue::starters();
    for (i, o) in spec.objects.iter().enumerate() {
        object(&mut c, o, &catalogue, &root.key("objects").index(i));
    }
    for (i, r) in spec.rooms.iter().enumerate() {
        room(&mut c, r, &root.key("rooms").index(i));
    }
    for (i, v) in spec.views.iter().enumerate() {
        view(&mut c, v, &root.key("views").index(i));
    }
    for (i, s) in spec.sheets.iter().enumerate() {
        sheet(&mut c, s, &root.key("sheets").index(i));
    }
    for (i, t) in spec.tags.iter().enumerate() {
        tag(&mut c, t, &root.key("tags").index(i));
    }
    for (i, n) in spec.notes.iter().enumerate() {
        note(&mut c, n, &root.key("notes").index(i));
    }

    if !c.errors.is_empty() {
        return Err(c.errors);
    }
    // a terrain's own `pads`, and its own few `points` and `lines`, are
    // the earlier way: each becomes an element (see `site::earlier_pads`)
    if let Some(tx) = site::earlier_pads(c.doc.model()) {
        let _ = c.doc.commit(tx);
    }
    let (model, _, _) = c.doc.into_parts();
    Ok(Built {
        model,
        warnings: c.warnings,
    })
}

const TAGGABLE: &[Tag] = &[
    Tag::Wall,
    Tag::Opening,
    Tag::Slab,
    Tag::Roof,
    Tag::Column,
    Tag::Beam,
    Tag::Stair,
    Tag::Railing,
    Tag::Part,
    Tag::Site,
    Tag::Object,
    Tag::Room,
];

fn tag(c: &mut Ctx, t: &f::Tag, at: &Path) {
    let refs = (
        c.get(&t.view, &[Tag::View], &at.key("view")),
        c.get(&t.element, TAGGABLE, &at.key("element")),
    );
    let (Some(view), Some(host)) = refs else {
        return;
    };
    let mut tag = core::Tag::new(view, host, c.p(t.offset));
    tag.elbow = t.elbow.map(|p| c.p(p));
    tag.end = t.end.map(|p| c.p(p));
    c.commit(
        "",
        Transaction::new("Build").insert(Element::new("Tag", ElementKind::Tag(tag))),
        at,
    );
}

fn note(c: &mut Ctx, n: &f::Note, at: &Path) {
    let Some(view) = c.get(&n.view, &[Tag::View, Tag::Sheet], &at.key("view")) else {
        return;
    };
    // on a sheet, paper millimetres; in a view, drawing units
    let on_sheet = matches!(c.ids.get(&n.view), Some((_, Tag::Sheet, _)));
    let p = |c: &Ctx, v: DVec2| if on_sheet { v } else { c.p(v) };
    let mut note = core::Note::new(view, p(c, n.at), n.text.clone());
    note.leader = n.leader.map(|l| core::HostPoint::free(p(c, l)));
    note.elbow = n.elbow.map(|e| p(c, e));
    note.color = n.colour.map(|col| col.0);
    note.symbol = n.symbol.as_ref().map(|s| core::PlacedSymbol {
        kind: s.kind.clone(),
        style: s.style.clone(),
        angle: s.angle,
    });
    let name = if note.symbol.is_some() {
        "Symbol"
    } else {
        "Note"
    };
    c.commit(
        "",
        Transaction::new("Build").insert(Element::new(name, ElementKind::Note(note))),
        at,
    );
}

/// Claim every id up front, so elements may refer to ones listed after
/// them (a wall attached to a roof).
fn claim_all(c: &mut Ctx, s: &f::Model, root: &Path) {
    macro_rules! claim {
        ($list:ident, $tag:expr) => {
            for (i, x) in s.$list.iter().enumerate() {
                c.claim(&x.id, $tag, &root.key(stringify!($list)).index(i));
            }
        };
    }
    claim!(materials, Tag::Material);
    claim!(assemblies, Tag::Assembly);
    for (i, t) in s.types.iter().enumerate() {
        c.claim(&t.id, Tag::Type(t.category), &root.key("types").index(i));
    }
    claim!(levels, Tag::Level);
    claim!(grid_groups, Tag::GridGroup);
    claim!(grids, Tag::Grid);
    claim!(walls, Tag::Wall);
    claim!(openings, Tag::Opening);
    claim!(slabs, Tag::Slab);
    claim!(roofs, Tag::Roof);
    claim!(columns, Tag::Column);
    claim!(beams, Tag::Beam);
    claim!(stairs, Tag::Stair);
    claim!(railings, Tag::Railing);
    claim!(parts, Tag::Part);
    claim!(site, Tag::Site);
    claim!(objects, Tag::Object);
    claim!(rooms, Tag::Room);
    claim!(views, Tag::View);
    claim!(sheets, Tag::Sheet);
}

fn hatch(h: f::Hatch) -> core::Hatch {
    match h {
        f::Hatch::Empty => core::Hatch::Empty,
        f::Hatch::Solid => core::Hatch::Solid,
        f::Hatch::Diagonal => core::Hatch::Diagonal,
        f::Hatch::Cross => core::Hatch::Cross,
        f::Hatch::Brick => core::Hatch::Brick,
        f::Hatch::Concrete => core::Hatch::Concrete,
        f::Hatch::Timber => core::Hatch::Timber,
    }
}

fn material(c: &mut Ctx, m: &f::Material, at: &Path) {
    let mut mat = match &m.library {
        None => core::Material::default(),
        Some(name) => match core::library::LIBRARY.iter().find(|e| e.name == name) {
            Some(e) => e.material(),
            None => {
                let names: Vec<&str> = core::library::LIBRARY.iter().map(|e| e.name).collect();
                let hint = suggest(name, &names)
                    .map(|s| format!("; did you mean \"{s}\"?"))
                    .unwrap_or_default();
                c.err(
                    &at.key("library"),
                    format!("no library material is called \"{name}\"{hint}"),
                );
                c.fail(&m.id);
                return;
            }
        },
    };
    if let Some(h) = m.hatch {
        mat.hatch = hatch(h);
    }
    if let Some(col) = m.colour {
        mat.color = col.0;
    }
    if let Some(s) = &m.category {
        mat.category = s.clone();
    }
    if let Some(s) = &m.description {
        mat.description = s.clone();
    }
    for (v, slot, key) in [
        (m.roughness, &mut mat.roughness, "roughness"),
        (m.metallic, &mut mat.metallic, "metallic"),
        (m.opacity, &mut mat.opacity, "opacity"),
    ] {
        if let Some(v) = v {
            if !(0.0..=1.0).contains(&v) {
                c.errors.push(Problem::new(
                    &at.key(key),
                    format!("must be from 0 to 1, got {v}"),
                ));
            }
            *slot = v as f32;
        }
    }
    if let Some(amount) = m.rate {
        if !(amount.is_finite() && amount >= 0.0) {
            c.errors.push(Problem::new(
                &at.key("rate"),
                format!("must be 0 or more, got {amount}"),
            ));
        }
        let per = match m.rate_per {
            None | Some(f::RatePer::SquareMetre) => core::takeoff::RateUnit::Area,
            Some(f::RatePer::CubicMetre) => core::takeoff::RateUnit::Volume,
            Some(f::RatePer::Metre) => core::takeoff::RateUnit::Length,
            Some(f::RatePer::Each) => core::takeoff::RateUnit::Each,
        };
        mat.rate = Some(core::takeoff::Rate { amount, per });
    }
    let name = m
        .name
        .clone()
        .or_else(|| m.library.clone())
        .unwrap_or_else(|| m.id.clone());
    c.insert(&m.id, Element::new(name, ElementKind::Material(mat)), at);
}

fn assembly(c: &mut Ctx, a: &f::Assembly, at: &Path) {
    let mut layers = vec![];
    let mut ok = true;
    for (i, l) in a.layers.iter().enumerate() {
        let lp = at.key("layers").index(i);
        let Some(material) = c.opt(&l.material, &[Tag::Material], &lp.key("material")) else {
            ok = false;
            continue;
        };
        layers.push(core::Layer {
            function: match l.function {
                f::LayerFunction::Structure => core::LayerFunction::Structure,
                f::LayerFunction::Substrate => core::LayerFunction::Substrate,
                f::LayerFunction::Insulation => core::LayerFunction::Insulation,
                f::LayerFunction::AirGap => core::LayerFunction::AirGap,
                f::LayerFunction::Membrane => core::LayerFunction::Membrane,
                f::LayerFunction::Finish => core::LayerFunction::Finish,
            },
            material,
            thickness: c.l(l.thickness),
            priority: match l.priority {
                Some(p @ 1..=5) => Some(p as u8),
                Some(p) => {
                    c.err(&lp.key("priority"), format!("must be from 1 to 5, got {p}"));
                    ok = false;
                    None
                }
                None => None,
            },
        });
    }
    if !ok {
        return c.fail(&a.id);
    }
    let asm = core::Assembly {
        usage: match a.usage {
            f::AssemblyUsage::Wall => core::AssemblyUse::Wall,
            f::AssemblyUsage::Floor => core::AssemblyUse::Floor,
            f::AssemblyUsage::Roof => core::AssemblyUse::Roof,
        },
        layers,
        description: a.description.clone().unwrap_or_default(),
    };
    c.insert(
        &a.id,
        Element::new(name_of(&a.name, &a.id), ElementKind::Assembly(asm)),
        at,
    );
}

fn location(l: f::Location, boundary: Option<u32>) -> core::LocationLine {
    match l {
        f::Location::Centre => core::LocationLine::Centre,
        f::Location::CoreCentre => core::LocationLine::CoreCentre,
        f::Location::ExteriorFace => core::LocationLine::ExteriorFace,
        f::Location::InteriorFace => core::LocationLine::InteriorFace,
        f::Location::CoreExterior => core::LocationLine::CoreExterior,
        f::Location::CoreInterior => core::LocationLine::CoreInterior,
        f::Location::LayerBoundary => {
            core::LocationLine::Boundary(boundary.unwrap_or(0).min(255) as u8)
        }
    }
}

fn frame(c: &Ctx, fr: &f::Frame, kind: core::OpeningKind) -> core::OpeningFrame {
    use core::cells as k;
    let grid = if fr.cells.is_empty() {
        k::Grid::of_operation(
            kind,
            match fr.operation.unwrap_or(f::Operation::Standard) {
                f::Operation::Standard => core::Operation::Standard,
                f::Operation::DoubleSwing => core::Operation::DoubleSwing,
                f::Operation::SurfaceSliding => core::Operation::SurfaceSliding,
                f::Operation::SurfaceSlidingPair => core::Operation::SurfaceSlidingPair,
                f::Operation::Pocket => core::Operation::Pocket,
                f::Operation::PocketPair => core::Operation::PocketPair,
                f::Operation::MultiSlide => core::Operation::MultiSlide,
            },
            fr.active_share.unwrap_or(0.5),
            fr.panels.unwrap_or(2),
            fr.fixed.as_ref().map_or(1, |v| {
                v.iter()
                    .filter(|i| (1..=32).contains(*i))
                    .fold(0, |m, i| m | 1 << (i - 1))
            }),
            [fr.panes_across.unwrap_or(1), fr.panes_up.unwrap_or(1)],
        )
    } else {
        let tracks = |t: &[f::Track]| -> Vec<k::Track> {
            t.iter()
                .map(|t| k::Track {
                    share: t.share,
                    bar: t.bar,
                })
                .collect()
        };
        k::Grid {
            outline: match fr.outline {
                f::Outline::Rectangle => k::Outline::Rectangle,
                f::Outline::Segmental => k::Outline::Segmental { rise: fr.rise },
                f::Outline::Round => k::Outline::Round,
                f::Outline::Raked => k::Outline::Raked { drop: fr.rise },
            },
            columns: tracks(&fr.columns),
            rows: tracks(&fr.rows),
            cells: fr
                .cells
                .iter()
                .map(|cl| k::Cell {
                    mechanism: match cl.mechanism {
                        f::Mechanism::Fixed => k::Mechanism::Fixed,
                        f::Mechanism::Hinged => k::Mechanism::Hinged,
                        f::Mechanism::TopHung => k::Mechanism::TopHung,
                        f::Mechanism::BottomHung => k::Mechanism::BottomHung,
                        f::Mechanism::Louvred => k::Mechanism::Louvred,
                        f::Mechanism::SurfaceSliding => k::Mechanism::SurfaceSliding,
                        f::Mechanism::Pocket => k::Mechanism::Pocket,
                        f::Mechanism::Sliding => k::Mechanism::Sliding,
                        f::Mechanism::Pivot => k::Mechanism::Pivot,
                        f::Mechanism::CentrePivot => k::Mechanism::CentrePivot,
                        f::Mechanism::DoubleAction => k::Mechanism::DoubleAction,
                        f::Mechanism::Folding => k::Mechanism::Folding,
                        f::Mechanism::TiltTurn => k::Mechanism::TiltTurn,
                        f::Mechanism::VerticalSliding => k::Mechanism::VerticalSliding,
                        f::Mechanism::HorizontalPivot => k::Mechanism::HorizontalPivot,
                        f::Mechanism::Sectional => k::Mechanism::Sectional,
                        f::Mechanism::Rolling => k::Mechanism::Rolling,
                        f::Mechanism::Revolving => k::Mechanism::Revolving,
                    },
                    hand: cl.hand,
                    flipped: cl.flipped,
                    active: cl.active,
                    track: cl.track.min(1) as u8,
                    display: match cl.display {
                        f::CellDisplay::Convention => k::Display::Convention,
                        f::CellDisplay::Closed => k::Display::Closed,
                        f::CellDisplay::Open => k::Display::Open,
                    },
                    panes: [cl.panes_across, cl.panes_up],
                    across: cl.across.max(1),
                })
                .collect(),
        }
        .fitted()
    };
    use creabim_core::construction as k2;
    core::OpeningFrame {
        width: c.l(fr.width),
        depth: c.l(fr.depth),
        grid,
        construction: k2::Construction {
            leaf: match fr.leaf {
                f::LeafStyle::Flush => k2::LeafStyle::Flush,
                f::LeafStyle::Panelled => k2::LeafStyle::Panelled,
                f::LeafStyle::Glazed => k2::LeafStyle::Glazed,
                f::LeafStyle::PartGlazed => k2::LeafStyle::PartGlazed,
                f::LeafStyle::Louvred => k2::LeafStyle::Louvred,
                f::LeafStyle::VisionPanel => k2::LeafStyle::VisionPanel,
                f::LeafStyle::AllGlass => k2::LeafStyle::AllGlass,
                f::LeafStyle::French => k2::LeafStyle::French,
                f::LeafStyle::SlimSteel => k2::LeafStyle::SlimSteel,
            },
            fittings: match fr.fittings {
                f::GlassFittings::Patch => k2::GlassFittings::Patch,
                f::GlassFittings::BottomRail => k2::GlassFittings::BottomRail,
                f::GlassFittings::Rails => k2::GlassFittings::Rails,
            },
            panes: match (fr.leaf_panes_across, fr.leaf_panes_up) {
                (Some(a), Some(u)) => Some([a.clamp(1, 12) as u8, u.clamp(1, 12) as u8]),
                _ => None,
            },
            trim: k2::Trim {
                lining: c.l(fr.lining),
                architrave: c.l(fr.architrave),
                threshold: c.l(fr.threshold),
            }
            .fitted(),
            ratings: k2::Ratings {
                load_bearing: None,
                external: None,
                fire: fr.fire_rating.clone().unwrap_or_default(),
                acoustic: fr.acoustic_rating.clone().unwrap_or_default(),
                security: fr.security_rating.clone().unwrap_or_default(),
                u_value: fr.u_value,
                accessible: fr.accessible,
            },
        },
    }
}

/// The total of assembly `id`'s layers, once it is in the model.
fn assembly_total(c: &Ctx, id: Option<ElementId>) -> Option<f64> {
    match c.model().get(id?).map(|e| &e.kind) {
        Some(ElementKind::Assembly(a)) if a.total() > 1e-9 => Some(a.total()),
        _ => None,
    }
}

/// Report the fields given in `v` that its kind doesn't use.
fn check_stray(c: &mut Ctx, v: &impl Spec, allowed: &[&str], kind: &str, at: &Path) -> bool {
    let extra = stray(v, allowed);
    for k in &extra {
        c.err(&at.key(k), format!("`{k}` is not used by {kind}"));
    }
    extra.is_empty()
}

fn element_type(c: &mut Ctx, t: &f::Type, at: &Path) {
    use f::TypeCategory as C;
    const COMMON: &[&str] = &["id", "name", "category", "description", "material"];
    let own: &[&str] = match t.category {
        C::Wall => &["thickness", "assembly", "location", "height", "screen"],
        C::Door | C::Window => &["width", "height", "sill", "frame"],
        C::Slab | C::Roof => &["thickness", "assembly"],
        C::Column => &["width", "depth", "diameter"],
        C::Beam => &["width", "depth"],
    };
    let allowed: Vec<&str> = COMMON.iter().chain(own).copied().collect();
    let kind = format!("a {} type", t.category.key());
    let mut ok = check_stray(c, t, &allowed, &kind, at);
    let Some(material) = c.opt(&t.material, &[Tag::Material], &at.key("material")) else {
        return c.fail(&t.id);
    };
    let use_as = match t.category {
        C::Roof => core::AssemblyUse::Roof,
        C::Slab => core::AssemblyUse::Floor,
        _ => core::AssemblyUse::Wall,
    };
    let _ = use_as;
    let Some(assembly) = c.opt(&t.assembly, &[Tag::Assembly], &at.key("assembly")) else {
        return c.fail(&t.id);
    };
    let mut need = |c: &mut Ctx, v: Option<f64>, key: &str| -> f64 {
        match v {
            Some(v) => c.l(v),
            None => {
                c.err(at, format!("{kind} needs `{key}`"));
                ok = false;
                0.0
            }
        }
    };
    let thick = |c: &mut Ctx, need: &mut dyn FnMut(&mut Ctx, Option<f64>, &str) -> f64| {
        match assembly_total(c, assembly) {
            Some(total) if t.thickness.is_none() => total,
            _ => need(c, t.thickness, "thickness (or an assembly)"),
        }
    };
    let category = match t.category {
        C::Wall => core::TypeCategory::Wall,
        C::Door => core::TypeCategory::Door,
        C::Window => core::TypeCategory::Window,
        C::Slab => core::TypeCategory::Slab,
        C::Roof => core::TypeCategory::Roof,
        C::Column => core::TypeCategory::Column,
        C::Beam => core::TypeCategory::Beam,
    };
    let mut params = types::default_params(category, false);
    match &mut params {
        core::TypeParams::Wall(w) => {
            w.thickness = thick(c, &mut need);
            w.assembly = assembly;
            w.material = material;
            w.location = t
                .location
                .map(|l| location(l, t.boundary))
                .unwrap_or_default();
            w.height = t.height.map(|h| c.l(h));
            w.curtain = None;
            w.screen = t.screen.as_ref().map(|s| {
                Box::new(core::ScreenPattern {
                    unit: c.p(s.unit),
                    holes: s.holes.iter().map(|r| [c.p(r.min), c.p(r.max)]).collect(),
                    frame: c.l(s.frame),
                    stagger: s.stagger,
                })
            });
        }
        core::TypeParams::Door(o) | core::TypeParams::Window(o) => {
            o.width = need(c, t.width, "width");
            o.height = need(c, t.height, "height");
            o.sill = t.sill.map(|s| c.l(s)).unwrap_or(match t.category {
                C::Window => 0.9,
                _ => 0.0,
            });
            let kind = match t.category {
                C::Window => core::OpeningKind::Window,
                _ => core::OpeningKind::Door,
            };
            o.frame = t.frame.as_ref().map_or_else(
                || core::OpeningFrame::for_kind(kind),
                |fr| frame(c, fr, kind),
            );
            o.material = material;
        }
        core::TypeParams::Slab(s) | core::TypeParams::Roof(s) => {
            s.thickness = thick(c, &mut need);
            s.assembly = assembly;
            s.material = material;
        }
        core::TypeParams::Column(col) => {
            match profile(c, t.width, t.depth, t.diameter, at) {
                Some(p) => col.profile = p,
                None => ok = false,
            }
            col.material = material;
        }
        core::TypeParams::Beam(b) => {
            b.width = need(c, t.width, "width");
            b.depth = need(c, t.depth, "depth");
            b.material = material;
        }
        _ => unreachable!("the format has no other categories"),
    }
    if !ok {
        return c.fail(&t.id);
    }
    let mut ty = core::ElementType::new(params);
    ty.description = t.description.clone().unwrap_or_default();
    c.insert(
        &t.id,
        Element::new(name_of(&t.name, &t.id), ElementKind::ElementType(ty)),
        at,
    );
}

/// A column profile from its width and depth, or its diameter.
fn profile(
    c: &mut Ctx,
    width: Option<f64>,
    depth: Option<f64>,
    diameter: Option<f64>,
    at: &Path,
) -> Option<core::Profile> {
    match (width, depth, diameter) {
        (Some(w), Some(d), None) => Some(core::Profile::Rectangle {
            width: c.l(w),
            depth: c.l(d),
        }),
        (None, None, Some(d)) => Some(core::Profile::Round { diameter: c.l(d) }),
        (_, _, Some(_)) => {
            c.err(at, "give `width` and `depth`, or `diameter`, not both");
            None
        }
        _ => {
            c.err(
                at,
                "needs `width` and `depth` (a rectangle) or `diameter` (round)",
            );
            None
        }
    }
}

/// Stamp `e` with the type `id`, if one is given. False after a problem.
fn stamp(
    c: &mut Ctx,
    e: &mut Element,
    id: &Option<String>,
    want: f::TypeCategory,
    at: &Path,
) -> bool {
    let Some(tid) = id else { return true };
    let Some(eid) = c.get(tid, &[Tag::Type(want)], &at.key("type")) else {
        return false;
    };
    if !types::stamp(c.model(), e, eid) {
        c.err(
            &at.key("type"),
            format!("\"{tid}\" doesn't fit this element"),
        );
        return false;
    }
    true
}

fn level_height(c: &Ctx, level: ElementId) -> (f64, f64) {
    c.model()
        .level(level)
        .map(|l| (l.elevation, l.height))
        .unwrap_or((0.0, 3.0))
}

fn wall(c: &mut Ctx, w: &f::Wall, at: &Path) {
    let Some(level) = c.get(&w.level, &[Tag::Level], &at.key("level")) else {
        return c.fail(&w.id);
    };
    let refs = (
        c.opt(&w.top_level, &[Tag::Level], &at.key("top_level")),
        c.opt(&w.material, &[Tag::Material], &at.key("material")),
        c.opt(&w.assembly, &[Tag::Assembly], &at.key("assembly")),
    );
    let (Some(top), Some(material), Some(assembly)) = refs else {
        return c.fail(&w.id);
    };
    let mut attach = vec![];
    for (i, a) in w.attach_top.iter().enumerate() {
        match c.get(a, &[Tag::Roof, Tag::Slab], &at.key("attach_top").index(i)) {
            Some(id) => attach.push(id),
            None => return c.fail(&w.id),
        }
    }
    let (elevation, storey) = level_height(c, level);
    let mut wall = core::Wall::new(level, c.p(w.start), c.p(w.end), 0.2, storey);
    wall.base_offset = c.l(w.base_offset);
    let mut e = Element::new(name_of(&w.name, &w.id), ElementKind::Wall(wall));
    if !stamp(c, &mut e, &w.r#type, f::TypeCategory::Wall, at) {
        return c.fail(&w.id);
    }
    let typed = w.r#type.is_some();
    if !typed && w.thickness.is_none() && w.assembly.is_none() {
        c.err(at, "a wall needs `thickness`, an `assembly` or a `type`");
        return c.fail(&w.id);
    }
    let ElementKind::Wall(wall) = &mut e.kind else {
        unreachable!()
    };
    if let Some(t) = w.thickness {
        wall.thickness = c.k * t;
    }
    if w.material.is_some() {
        wall.material = material;
    }
    if let Some(l) = w.location {
        wall.location = location(l, w.boundary);
    }
    wall.flip = w.flip;
    wall.attach_top = attach;
    if let Some(t) = top {
        let top_elevation = c.model().level(t).map(|l| l.elevation).unwrap_or(0.0);
        wall.top = core::WallTop::UpToLevel {
            level: t,
            offset: c.l(w.top_offset),
        };
        wall.height = top_elevation + c.l(w.top_offset) - elevation - wall.base_offset;
    } else if w.top_offset != 0.0 {
        c.err(&at.key("top_offset"), "`top_offset` needs a `top_level`");
    }
    if let Some(h) = w.height {
        if top.is_some() {
            c.err(&at.key("height"), "give `height` or `top_level`, not both");
        }
        wall.height = c.l(h);
    }
    if w.assembly.is_some() {
        core::assembly::set_assembly(c.model(), &mut e, assembly);
    }
    e.mark = w.mark.clone();
    c.insert(&w.id, e, at);
}

fn opening(c: &mut Ctx, o: &f::Opening, at: &Path) {
    // a roof window is set in a roof, the rest in a wall
    let hosts: &[Tag] = if o.roof_at.is_some() {
        &[Tag::Roof]
    } else {
        &[Tag::Wall]
    };
    let Some(host) = c.get(&o.wall, hosts, &at.key("wall")) else {
        return c.fail(&o.id);
    };
    let Some(material) = c.opt(&o.material, &[Tag::Material], &at.key("material")) else {
        return c.fail(&o.id);
    };
    if o.roof_at.is_some() && o.kind != f::OpeningKind::Window {
        c.err(&at.key("roof_at"), "only a window can be set in a roof");
        return c.fail(&o.id);
    }
    let w = match c.model().get(host).map(|e| e.kind.clone()) {
        Some(ElementKind::Wall(w)) => Some(w),
        Some(ElementKind::Roof(_)) if o.roof_at.is_some() => None,
        _ => return c.fail(&o.id),
    };
    let kind = match o.kind {
        f::OpeningKind::Door => core::OpeningKind::Door,
        f::OpeningKind::Window => core::OpeningKind::Window,
        f::OpeningKind::Hole => core::OpeningKind::Hole,
    };
    let opening = core::Opening {
        host,
        kind,
        offset: 0.0,
        sill: if kind == core::OpeningKind::Window {
            0.9
        } else {
            0.0
        },
        width: 0.9,
        height: 2.1,
        material: None,
        frame: creabim_core::OpeningFrame::for_kind(kind),
        flip_hand: o.flip_hand,
        flip_facing: o.flip_facing,
        roof_at: o.roof_at.map(|p| p * c.k),
        roof_build: Default::default(),
        arrangement: o.arrangement.as_ref().map(arrangement),
    };
    let mut e = Element::new(name_of(&o.name, &o.id), ElementKind::Opening(opening));
    let typed = match o.kind {
        f::OpeningKind::Door => stamp(c, &mut e, &o.r#type, f::TypeCategory::Door, at),
        f::OpeningKind::Window => stamp(c, &mut e, &o.r#type, f::TypeCategory::Window, at),
        f::OpeningKind::Hole => {
            if o.r#type.is_some() {
                c.err(&at.key("type"), "a hole has no type");
                false
            } else {
                true
            }
        }
    };
    if !typed {
        return c.fail(&o.id);
    }
    if o.r#type.is_none() {
        for (v, key) in [(o.width, "width"), (o.height, "height")] {
            if v.is_none() {
                c.err(at, format!("an opening needs `{key}` or a `type`"));
                return c.fail(&o.id);
            }
        }
    }
    let ElementKind::Opening(op) = &mut e.kind else {
        unreachable!()
    };
    if let Some(v) = o.width {
        op.width = c.k * v;
    }
    if let Some(v) = o.height {
        op.height = c.k * v;
    }
    if let Some(v) = o.sill {
        op.sill = c.k * v;
    }
    if let Some(fr) = &o.frame {
        op.frame = frame(c, fr, op.kind);
    }
    if o.material.is_some() {
        op.material = material;
    }
    let Some(w) = w else {
        // a roof window: placed by its centre in plan
        op.sill = 0.0;
        op.offset = 0.0;
        e.mark = o.mark.clone();
        c.insert(&o.id, e, at);
        return;
    };
    op.offset = match (o.offset, o.centre) {
        (Some(v), None) => c.k * v,
        (None, Some(m)) => c.k * m - op.width / 2.0,
        (Some(_), Some(_)) => {
            c.err(at, "give `offset` or `centre`, not both");
            return c.fail(&o.id);
        }
        (None, None) => {
            c.err(
                at,
                "an opening needs `offset` (to its near edge) or `centre`",
            );
            return c.fail(&o.id);
        }
    };
    let len = w.length();
    if op.offset < -1e-9 || op.offset + op.width > len + 1e-6 {
        c.err(
            at,
            format!(
                "it runs from {:.3} to {:.3} m along its wall, which is {:.3} m long",
                op.offset,
                op.offset + op.width,
                len
            ),
        );
        return c.fail(&o.id);
    }
    op.offset = op.offset.max(0.0);
    if op.sill + op.height > w.height + 1e-6 {
        c.warnings.push(Problem::new(
            at,
            format!(
                "its top ({:.3} m) is above its wall's ({:.3} m)",
                op.sill + op.height,
                w.height
            ),
        ));
    }
    e.mark = o.mark.clone();
    c.insert(&o.id, e, at);
}

fn slab(c: &mut Ctx, s: &f::Slab, at: &Path) {
    let refs = (
        c.get(&s.level, &[Tag::Level], &at.key("level")),
        c.opt(&s.material, &[Tag::Material], &at.key("material")),
        c.opt(&s.assembly, &[Tag::Assembly], &at.key("assembly")),
    );
    let (Some(level), Some(material), Some(assembly)) = refs else {
        return c.fail(&s.id);
    };
    let mut slab = core::Slab::new(level, c.pts(&s.outline), 0.2);
    slab.bulges = s.bulges.clone();
    slab.holes = s.holes.iter().map(|h| c.pts(h)).collect();
    slab.make_ccw();
    let mut e = Element::new(name_of(&s.name, &s.id), ElementKind::Slab(slab));
    if !stamp(c, &mut e, &s.r#type, f::TypeCategory::Slab, at) {
        return c.fail(&s.id);
    }
    if s.r#type.is_none() && s.thickness.is_none() && s.assembly.is_none() {
        c.err(at, "a slab needs `thickness`, an `assembly` or a `type`");
        return c.fail(&s.id);
    }
    let ElementKind::Slab(slab) = &mut e.kind else {
        unreachable!()
    };
    if let Some(t) = s.thickness {
        slab.thickness = c.k * t;
    }
    slab.top_offset = c.k * s.top_offset;
    if s.material.is_some() {
        slab.material = material;
    }
    if s.assembly.is_some() {
        core::assembly::set_assembly(c.model(), &mut e, assembly);
    }
    c.insert(&s.id, e, at);
}

fn roof(c: &mut Ctx, r: &f::Roof, at: &Path) {
    let refs = (
        c.get(&r.level, &[Tag::Level], &at.key("level")),
        c.opt(&r.material, &[Tag::Material], &at.key("material")),
        c.opt(&r.assembly, &[Tag::Assembly], &at.key("assembly")),
    );
    let (Some(level), Some(material), Some(assembly)) = refs else {
        return c.fail(&r.id);
    };
    let mut boundary = c.pts(&r.outline);
    let n = boundary.len();
    let mut edge = r.edge as usize;
    if n >= 3 && edge >= n {
        c.err(
            &at.key("edge"),
            format!("the outline has {n} edges, numbered from 0"),
        );
        return c.fail(&r.id);
    }
    // kept counter-clockwise; the edge keeps naming the same side
    if n >= 3 && core::polygon::signed_area2(&boundary) < 0.0 {
        boundary.reverse();
        edge = (2 * n - 2 - edge) % n;
    }
    let shape = match r.shape {
        f::RoofShape::Flat => core::RoofShape::Flat,
        f::RoofShape::Shed => core::RoofShape::Shed,
        f::RoofShape::Gable => core::RoofShape::Gable,
        f::RoofShape::Hip => core::RoofShape::Hip,
    };
    let pitch = match (r.pitch, shape) {
        (Some(p), _) => p,
        (None, core::RoofShape::Flat) => 0.0,
        (None, _) => {
            c.err(at, format!("a {} roof needs `pitch`", r.shape.key()));
            return c.fail(&r.id);
        }
    };
    let roof = core::Roof {
        layer_edges: vec![],
        level,
        boundary,
        shape,
        pitch,
        overhang: c.l(r.overhang),
        thickness: r.thickness.map(|t| c.l(t)).unwrap_or(0.2),
        base_offset: c.l(r.base_offset),
        edge,
        material: None,
        assembly: None,
        holes: r.holes.iter().map(|h| c.pts(h)).collect(),
    };
    let mut e = Element::new(name_of(&r.name, &r.id), ElementKind::Roof(roof));
    if !stamp(c, &mut e, &r.r#type, f::TypeCategory::Roof, at) {
        return c.fail(&r.id);
    }
    if r.r#type.is_none() && r.thickness.is_none() && r.assembly.is_none() {
        c.err(at, "a roof needs `thickness`, an `assembly` or a `type`");
        return c.fail(&r.id);
    }
    let ElementKind::Roof(roof) = &mut e.kind else {
        unreachable!()
    };
    if let Some(t) = r.thickness {
        roof.thickness = c.k * t;
    }
    if r.material.is_some() {
        roof.material = material;
    }
    if r.assembly.is_some() {
        core::assembly::set_assembly(c.model(), &mut e, assembly);
    }
    c.insert(&r.id, e, at);
}

fn column(c: &mut Ctx, col: &f::Column, at: &Path) {
    let refs = (
        c.get(&col.level, &[Tag::Level], &at.key("level")),
        c.opt(&col.top_level, &[Tag::Level], &at.key("top_level")),
        c.opt(&col.material, &[Tag::Material], &at.key("material")),
    );
    let (Some(level), Some(top), Some(material)) = refs else {
        return c.fail(&col.id);
    };
    let column = core::Column {
        level,
        position: c.p(col.at),
        profile: core::Profile::Rectangle {
            width: 0.3,
            depth: 0.3,
        },
        rotation: col.rotation,
        base_offset: c.l(col.base_offset),
        height: col.height.map(|h| c.l(h)),
        material: None,
        section: None,
        top: match top {
            Some(t) => core::WallTop::UpToLevel {
                level: t,
                offset: c.l(col.top_offset),
            },
            None => core::WallTop::Unconnected,
        },
    };
    let mut e = Element::new(name_of(&col.name, &col.id), ElementKind::Column(column));
    if !stamp(c, &mut e, &col.r#type, f::TypeCategory::Column, at) {
        return c.fail(&col.id);
    }
    let given = col.width.is_some() || col.depth.is_some() || col.diameter.is_some();
    let profile = if given || col.r#type.is_none() {
        match profile(c, col.width, col.depth, col.diameter, at) {
            Some(p) => Some(p),
            None => return c.fail(&col.id),
        }
    } else {
        None
    };
    let ElementKind::Column(column) = &mut e.kind else {
        unreachable!()
    };
    if let Some(p) = profile {
        column.profile = p;
    }
    if col.material.is_some() {
        column.material = material;
    }
    e.mark = col.mark.clone();
    c.insert(&col.id, e, at);
}

fn beam(c: &mut Ctx, b: &f::Beam, at: &Path) {
    let refs = (
        c.get(&b.level, &[Tag::Level], &at.key("level")),
        c.opt(&b.material, &[Tag::Material], &at.key("material")),
    );
    let (Some(level), Some(material)) = refs else {
        return c.fail(&b.id);
    };
    let beam = core::Beam {
        level,
        start: c.p(b.start),
        end: c.p(b.end),
        width: 0.2,
        depth: 0.4,
        top_offset: c.l(b.top_offset),
        material: None,
        section: None,
        rise: [0.0, 0.0],
        up: None,
        extend: [0.0, 0.0],
        cuts: [None, None],
    };
    let mut e = Element::new(name_of(&b.name, &b.id), ElementKind::Beam(beam));
    if !stamp(c, &mut e, &b.r#type, f::TypeCategory::Beam, at) {
        return c.fail(&b.id);
    }
    if b.r#type.is_none() && (b.width.is_none() || b.depth.is_none()) {
        c.err(at, "a beam needs `width` and `depth`, or a `type`");
        return c.fail(&b.id);
    }
    let ElementKind::Beam(beam) = &mut e.kind else {
        unreachable!()
    };
    if let Some(v) = b.width {
        beam.width = c.k * v;
    }
    if let Some(v) = b.depth {
        beam.depth = c.k * v;
    }
    if b.material.is_some() {
        beam.material = material;
    }
    e.mark = b.mark.clone();
    c.insert(&b.id, e, at);
}

fn stair(c: &mut Ctx, s: &f::Stair, at: &Path) {
    let refs = (
        c.get(&s.level, &[Tag::Level], &at.key("level")),
        c.opt(&s.material, &[Tag::Material], &at.key("material")),
    );
    let fat = at.key("finish");
    let fin = s.finish.as_ref();
    let none = None;
    let fins = (
        c.opt(
            fin.map_or(&none, |f| &f.tread_material),
            &[Tag::Material],
            &fat.key("tread_material"),
        ),
        c.opt(
            fin.map_or(&none, |f| &f.riser_material),
            &[Tag::Material],
            &fat.key("riser_material"),
        ),
    );
    let typical = c.opt(&s.typical, &[Tag::Stair], &at.key("typical"));
    let (Some(level), Some(material), (Some(tread_material), Some(riser_material)), Some(typical)) =
        (refs.0, refs.1, fins, typical)
    else {
        return c.fail(&s.id);
    };
    let turn_kind = |k: f::StairTurnKind, n: u32| match k {
        f::StairTurnKind::Landing => core::members::TurnKind::Landing,
        f::StairTurnKind::Winders => core::members::TurnKind::Winders(n),
    };
    let mut stair = core::Stair {
        level,
        start: c.p(s.start),
        direction: s.direction,
        width: c.l(s.width),
        max_riser: c.l(s.max_riser),
        tread: c.l(s.tread),
        height: s.height.map(|h| c.l(h)),
        base_offset: c.l(s.base_offset),
        waist: c.l(s.waist),
        landing: s.landing.as_ref().map(|l| core::Landing {
            after: l.after,
            left: l.turn == f::Side::Left,
        }),
        material,
        line: vec![],
        bends: vec![],
        drawn: match s.drawn {
            f::StairLine::Walk => core::members::StairLine::Walk,
            f::StairLine::Left => core::members::StairLine::Left,
            f::StairLine::Right => core::members::StairLine::Right,
        },
        pieces: core::members::StairPieces {
            flights: s
                .flights
                .iter()
                .map(|f| core::members::FlightEdit {
                    straight: f.straight,
                    treads: f.treads,
                })
                .collect(),
            landings: s
                .landings
                .iter()
                .map(|l| core::members::LandingEdit {
                    turn: l.turn,
                    deeper: c.l(l.deeper),
                })
                .collect(),
            turns: s
                .turn_kinds
                .iter()
                .map(|t| core::members::TurnEdit {
                    turn: t.turn,
                    kind: turn_kind(t.kind, t.winders),
                })
                .collect(),
        },
        at_turns: turn_kind(s.turns, s.winders),
        rules: core::stairrules::StairCode::from_key(&s.rules),
        ramp: None,
        typical,
        split: s.split,
        // not in the text format yet: the head stays, and nothing kept from a ramp
        fixed_foot: false,
        as_steps: None,
        up_offset: s.up_label.map(|v| glam::DVec2::new(c.l(v.x), c.l(v.y))),
        build: {
            use core::members::StairBuild as B;
            match s.build {
                f::StairBuild::Concrete => B::Concrete,
                f::StairBuild::Stepped => B::Stepped,
                f::StairBuild::FoldedPlate => B::FoldedPlate,
                f::StairBuild::Stringers => B::Stringers,
                f::StairBuild::CutStringers => B::CutStringers,
                f::StairBuild::Spine => B::Spine,
                f::StairBuild::Cantilever => B::Cantilever,
                f::StairBuild::Post => B::Post,
            }
        },
        finish: core::members::StairFinish {
            tread: fin.map_or(0.0, |f| c.l(f.tread)),
            tread_material,
            riser: fin.map_or(0.0, |f| c.l(f.riser)),
            riser_material,
            nosing: fin.map_or(0.0, |f| c.l(f.nosing)),
        },
    };
    if let Some(slope) = s.ramp {
        stair.set_slope(slope);
    }
    if s.line.len() >= 2 {
        let pts: Vec<_> = s.line.iter().map(|p| c.p(*p)).collect();
        stair.set_line(&pts);
        for (i, b) in s.bends.iter().enumerate() {
            stair.set_bend(i, *b);
        }
    }
    c.insert(
        &s.id,
        Element::new(name_of(&s.name, &s.id), ElementKind::Stair(stair)),
        at,
    );
}

fn railing(c: &mut Ctx, r: &f::Railing, at: &Path) {
    let refs = (
        c.get(&r.level, &[Tag::Level], &at.key("level")),
        c.opt(&r.stair, &[Tag::Stair], &at.key("stair")),
        c.opt(&r.material, &[Tag::Material], &at.key("material")),
    );
    let (Some(level), Some(stair), Some(material)) = refs else {
        return c.fail(&r.id);
    };
    let stair = match (stair, r.side) {
        (Some(s), Some(side)) => Some(core::StairSide {
            stair: s,
            left: side == f::Side::Left,
        }),
        (Some(_), None) => {
            c.err(
                at,
                "a railing on a stair needs `side`: \"left\" or \"right\"",
            );
            return c.fail(&r.id);
        }
        (None, Some(_)) => {
            c.err(&at.key("side"), "`side` needs a `stair`");
            return c.fail(&r.id);
        }
        (None, None) => {
            if r.path.len() < 2 {
                c.err(
                    at,
                    "a railing needs a `path` of at least 2 points, or a `stair`",
                );
                return c.fail(&r.id);
            }
            None
        }
    };
    let railing = core::Railing {
        level,
        path: r.path.iter().map(|p| c.p3(*p)).collect(),
        height: c.l(r.height),
        post_spacing: c.l(r.post_spacing),
        stair,
        material,
        rail_profile: None,
        extend: r.extend.as_ref().map(|e| core::members::RailExtend {
            bottom: c.l(e.bottom),
            top: c.l(e.top),
            slope_bottom: e.slope_bottom,
        }),
        on_wall: r.on_wall,
        infill: match r.infill {
            f::RailInfill::Rail => core::members::RailInfill::Rail,
            f::RailInfill::Balusters => core::members::RailInfill::Balusters,
            f::RailInfill::Glass => core::members::RailInfill::Glass,
        },
        guard_height: r.guard_height.map(|h| c.l(h)),
        newels: r.newels,
    };
    c.insert(
        &r.id,
        Element::new(name_of(&r.name, &r.id), ElementKind::Railing(railing)),
        at,
    );
}

fn part(c: &mut Ctx, p: &f::Part, at: &Path) {
    let refs = (
        c.get(&p.level, &[Tag::Level], &at.key("level")),
        c.opt(&p.wall, &[Tag::Wall], &at.key("wall")),
        c.opt(&p.material, &[Tag::Material], &at.key("material")),
    );
    let (Some(level), Some(wall), Some(material)) = refs else {
        return c.fail(&p.id);
    };
    const COMMON: &[&str] = &["id", "name", "level", "void", "material", "ifc_class"];
    let extrusion = !p.outline.is_empty();
    let own: &[&str] = if extrusion {
        &["outline", "holes", "base", "height", "plane"]
    } else if p.wall.is_some() {
        &["profile", "wall", "edge", "offset", "extend"]
    } else {
        &["profile", "path", "closed", "face_normal"]
    };
    let allowed: Vec<&str> = COMMON.iter().chain(own).copied().collect();
    let kind = if extrusion { "an extrusion" } else { "a sweep" };
    if !check_stray(c, p, &allowed, kind, at) {
        return c.fail(&p.id);
    }
    let mut part = if extrusion {
        let Some(h) = p.height else {
            c.err(at, "an extrusion needs `height`");
            return c.fail(&p.id);
        };
        let mut part = parts::Part::extrusion(level, c.pts(&p.outline), c.l(p.base), c.l(h));
        if let parts::Form::Extrusion { holes, plane, .. } = &mut part.form {
            *holes = p.holes.iter().map(|h| c.pts(h)).collect();
            if let Some(pl) = &p.plane {
                let (x, y) = (pl.x.normalize_or_zero(), pl.y.normalize_or_zero());
                if x == glam::DVec3::ZERO || y == glam::DVec3::ZERO || x.dot(y).abs() > 1e-6 {
                    c.err(&at.key("plane"), "a plane needs two square axes");
                    return c.fail(&p.id);
                }
                *plane = Some(creabim_core::Workplane {
                    origin: c.p3(pl.origin),
                    x,
                    y,
                });
            }
        }
        part
    } else if p.profile.is_empty() {
        c.err(
            at,
            "a part needs an `outline` (an extrusion) or a `profile` (a sweep)",
        );
        return c.fail(&p.id);
    } else {
        let path = match wall {
            Some(w) => parts::Path::Wall {
                wall: w,
                edge: match p.edge {
                    f::WallEdge::Top => parts::WallEdge::Top,
                    f::WallEdge::Base => parts::WallEdge::Base,
                },
                offset: c.l(p.offset),
                extend: p.extend.map(|x| c.p(x).to_array()).unwrap_or([0.0; 2]),
            },
            None => {
                if p.path.len() < 2 {
                    c.err(
                        at,
                        "a sweep needs a `path` of at least 2 points, or a `wall`",
                    );
                    return c.fail(&p.id);
                }
                let normal = p.face_normal.and_then(|n| n.try_normalize());
                if p.face_normal.is_some() && normal.is_none() {
                    c.err(&at.key("face_normal"), "a face normal needs a direction");
                    return c.fail(&p.id);
                }
                parts::Path::Points {
                    points: p.path.iter().map(|x| c.p3(*x)).collect(),
                    closed: p.closed,
                    normal,
                }
            }
        };
        parts::Part::sweep(level, c.pts(&p.profile), path)
    };
    part.void = p.void;
    part.material = material;
    if let Some(class) = &p.ifc_class {
        part.ifc_class = class.clone();
    }
    let name = p.name.clone().unwrap_or_else(|| p.id.clone());
    c.insert(&p.id, Element::new(name, ElementKind::Part(part)), at);
}

fn site_element(c: &mut Ctx, s: &f::SiteElement, at: &Path) {
    let refs = (
        c.get(&s.level, &[Tag::Level], &at.key("level")),
        c.opt(&s.material, &[Tag::Material], &at.key("material")),
    );
    let (Some(level), Some(material)) = refs else {
        return c.fail(&s.id);
    };
    const COMMON: &[&str] = &["id", "name", "kind", "level", "material"];
    let own: &[&str] = match s.kind {
        f::SiteKind::Terrain => &[
            "outline",
            "top",
            "thickness",
            "points",
            "contour",
            "lines",
            "survey",
            "faceted",
            "major",
            "labels",
            "levels",
            "shift",
            "pads",
            "under_buildings",
            "strata",
            "analysis",
            "flow",
        ],
        f::SiteKind::Area => &["outline", "holes", "top", "thickness", "pattern"],
        f::SiteKind::Pad => &[
            "outline",
            "top",
            "thickness",
            "cut",
            "fill",
            "depth",
            "width",
            "path",
            "only_cut",
            "retained",
            "radius",
        ],
        f::SiteKind::Plant => &["at", "base", "plant", "canopy", "height"],
        f::SiteKind::Property => &["outline", "edges", "rules"],
        f::SiteKind::Road => &["path", "road"],
        f::SiteKind::Drain => &["drain"],
        f::SiteKind::Tank => &["at", "tank"],
        f::SiteKind::Path => &["walk"],
        f::SiteKind::Investigation => &["at", "investigation"],
        f::SiteKind::Spot => &["at", "top"],
        f::SiteKind::GroundLine => &["lines"],
    };
    let allowed: Vec<&str> = COMMON.iter().chain(own).copied().collect();
    let kind = format!("a site {}", s.kind.key());
    if !check_stray(c, s, &allowed, &kind, at) {
        return c.fail(&s.id);
    }
    let missing = |c: &mut Ctx, v: Option<f64>, key: &str| -> Option<f64> {
        if v.is_none() {
            c.err(at, format!("{kind} needs `{key}`"));
        }
        v.map(|v| c.l(v))
    };
    let form = match s.kind {
        f::SiteKind::Terrain | f::SiteKind::Area => {
            let Some(thickness) = missing(c, s.thickness, "thickness") else {
                return c.fail(&s.id);
            };
            let mut boundary = c.pts(&s.outline);
            if core::polygon::signed_area2(&boundary) < 0.0 {
                boundary.reverse();
            }
            if s.kind == f::SiteKind::Terrain {
                site::SiteForm::Terrain {
                    boundary,
                    top: c.l(s.top),
                    thickness,
                    points: s.points.iter().map(|p| c.p3(*p)).collect(),
                    contour: c.l(s.contour),
                    data: site::GroundData {
                        lines: s
                            .lines
                            .iter()
                            .map(|l| site::GroundLine {
                                kind: if l.sharp {
                                    site::GroundLineKind::Break
                                } else {
                                    site::GroundLineKind::Contour
                                },
                                points: l.points.iter().map(|p| c.p3(*p)).collect(),
                                closed: l.closed,
                            })
                            .collect(),
                        survey: s.survey,
                        faceted: s.faceted,
                        major: s.major,
                        labels: s.labels,
                        levels: s.levels,
                        // a survey's own coordinates, in its own units
                        shift: s.shift.unwrap_or_default(),
                        pads: s
                            .pads
                            .iter()
                            .map(|p| site::Pad {
                                outline: p.outline.iter().map(|q| c.p(*q)).collect(),
                                level: c.l(p.level),
                                cut: p.cut,
                                fill: p.fill,
                                only_cut: false,
                                depth: c.l(p.depth),
                                heights: vec![],
                                sunk: 0.0,
                                upright: vec![],
                                behind: vec![],
                            })
                            .collect(),
                        under_buildings: s.under_buildings,
                        strata: s
                            .strata
                            .iter()
                            .map(|l| site::Stratum {
                                name: l.name.clone(),
                                thickness: c.l(l.thickness),
                            })
                            .collect(),
                        analysis: match s.analysis {
                            f::GroundShading::None => site::GroundAnalysis::None,
                            f::GroundShading::Slope => site::GroundAnalysis::Slope,
                            f::GroundShading::Aspect => site::GroundAnalysis::Aspect,
                            f::GroundShading::Height => site::GroundAnalysis::Height,
                        },
                        flow: s.flow,
                    },
                }
            } else {
                let pattern = match &s.pattern {
                    None => site::Pattern::None,
                    Some(p) => match pattern(c, p, &at.key("pattern")) {
                        Some(p) => p,
                        None => return c.fail(&s.id),
                    },
                };
                site::SiteForm::Area {
                    boundary,
                    holes: s.holes.iter().map(|h| c.pts(h)).collect(),
                    top: c.l(s.top),
                    thickness,
                    pattern,
                    parking: None,
                }
            }
        }
        f::SiteKind::Spot => {
            let Some(position) = s.at else {
                c.err(at, "a spot level needs `at`");
                return c.fail(&s.id);
            };
            site::SiteForm::Spot {
                position: c.p(position),
                height: c.l(s.top),
            }
        }
        f::SiteKind::GroundLine => {
            let [l] = s.lines.as_slice() else {
                c.err(at, "a ground line needs one line in `lines`");
                return c.fail(&s.id);
            };
            site::SiteForm::Line(site::GroundLine {
                kind: if l.sharp {
                    site::GroundLineKind::Break
                } else {
                    site::GroundLineKind::Contour
                },
                points: l.points.iter().map(|p| c.p3(*p)).collect(),
                closed: l.closed,
            })
        }
        f::SiteKind::Pad => {
            let mut boundary = c.pts(&s.outline);
            if core::polygon::signed_area2(&boundary) < 0.0 {
                boundary.reverse();
            }
            let flat: Vec<DVec2> = s.path.iter().map(|p| p.truncate()).collect();
            let path = c.pts(&flat);
            let width = c.l(s.width);
            let bend = c.l(s.radius);
            if path.len() >= 2 && width > 0.0 {
                boundary = site::trench_round(&path, bend, width);
                if core::polygon::signed_area2(&boundary) < 0.0 {
                    boundary.reverse();
                }
            }
            let depth = c.l(s.depth);
            let dug = depth > 0.0;
            site::SiteForm::Pad {
                boundary,
                top: c.l(s.top),
                // a pad with no thickness given keeps the base course it
                // always had; a pit or a trench has none
                base: s.thickness.map_or(if dug { 0.0 } else { 0.15 }, |t| c.l(t)),
                cut: s.cut.unwrap_or(if dug { 0.5 } else { 1.5 }),
                fill: s.fill.unwrap_or(if dug { 0.0 } else { 2.0 }),
                depth,
                path: if width > 0.0 { path } else { vec![] },
                width: if s.path.len() >= 2 { width } else { 0.0 },
                only_cut: s.only_cut,
                retained: s.retained.iter().map(|e| *e as usize).collect(),
                // a wall on a retained edge is an element of its own
                walls: vec![],
                bend,
            }
        }
        f::SiteKind::Plant => {
            let (Some(position), Some(canopy), Some(height)) = (
                s.at,
                missing(c, s.canopy, "canopy"),
                missing(c, s.height, "height"),
            ) else {
                if s.at.is_none() {
                    c.err(at, "a plant needs `at`");
                }
                return c.fail(&s.id);
            };
            site::SiteForm::Plant {
                position: c.p(position),
                base: c.l(s.base),
                form: match s.plant {
                    f::PlantForm::Tree => core::PlantForm::Tree,
                    f::PlantForm::Conifer => core::PlantForm::Conifer,
                    f::PlantForm::Palm => core::PlantForm::Palm,
                    f::PlantForm::Shrub => core::PlantForm::Shrub,
                },
                canopy,
                height,
                tree: None,
            }
        }
        f::SiteKind::Property => {
            let r = s.rules.clone().unwrap_or_else(|| f::LotRules {
                front: 6.0,
                side: 3.0,
                rear: 6.0,
                height: 0.0,
                coverage: 0.0,
                floor_ratio: 0.0,
                bearings: true,
                setbacks: true,
                angle: 0.0,
                angle_from: 2.5,
                envelope: true,
                easements: vec![],
                own: vec![],
            });
            let mut boundary = c.pts(&s.outline);
            let mut edges = s
                .edges
                .iter()
                .map(|e| match e {
                    f::LotEdge::Front => core::LotEdge::Front,
                    f::LotEdge::Side => core::LotEdge::Side,
                    f::LotEdge::Rear => core::LotEdge::Rear,
                    f::LotEdge::Open => core::LotEdge::Open,
                })
                .collect();
            let mut easements: Vec<core::Easement> = r
                .easements
                .iter()
                .map(|e| core::Easement {
                    name: e.name.clone(),
                    edge: e.edge as usize,
                    width: c.l(e.width),
                })
                .collect();
            let mut own: Vec<core::lot::EdgeRule> = r
                .own
                .iter()
                .map(|o| core::lot::EdgeRule {
                    edge: o.edge as usize,
                    setback: o.setback.map(|v| c.l(v)),
                    note: o.note.clone().unwrap_or_default(),
                    note_off: o.note_offset.map(|v| DVec2::new(c.l(v.x), c.l(v.y))),
                })
                .collect();
            core::lot::make_ccw(&mut boundary, &mut edges, &mut easements, &mut own);
            site::SiteForm::Property {
                boundary,
                edges,
                rules: core::PlanningRules {
                    front: c.l(r.front),
                    side: c.l(r.side),
                    rear: c.l(r.rear),
                    height: c.l(r.height),
                    coverage: r.coverage,
                    floor_ratio: r.floor_ratio,
                    bearings: r.bearings,
                    setbacks: r.setbacks,
                    has_setbacks: true,
                    angle: r.angle,
                    angle_from: c.l(r.angle_from),
                    envelope: r.envelope,
                    easements,
                    own,
                },
            }
        }
        f::SiteKind::Road => {
            if s.path.len() < 2 {
                c.err(at, "a road needs a `path` of two points");
                return c.fail(&s.id);
            }
            let r = s.road.clone().unwrap_or_default();
            let parking =
                r.parking
                    .as_ref()
                    .map_or_else(Default::default, |p| core::road::Parking {
                        side: match p.side {
                            f::ParkingSide::Left => core::road::Side::Left,
                            f::ParkingSide::Right => core::road::Side::Right,
                            f::ParkingSide::Both => core::road::Side::Both,
                        },
                        width: c.l(p.width),
                        length: c.l(p.length),
                        angle: p.angle,
                    });
            site::SiteForm::Road(core::road::Road {
                path: s.path.iter().map(|p| c.p3(*p)).collect(),
                width: c.l(r.width),
                fall: r.fall,
                crowned: r.crowned,
                kerb: c.l(r.kerb),
                footpath: c.l(r.footpath),
                thickness: c.l(r.thickness),
                cut: r.cut,
                fill: r.fill,
                works: r.works,
                parking,
                gullies: c.l(r.gullies),
                // the format gives the radius on the middle line; the
                // road keeps its inside kerb's, which bends the same
                radius: 0.0,
                kerb_radius: Some((c.l(r.radius) - c.l(r.width) / 2.0).max(0.0)),
                held: core::road::Side::None,
                start_at: None,
                end_at: None,
                levels: core::road::Levels::Points,
                profile: vec![],
                profile_key: 0,
            })
        }
        f::SiteKind::Path => {
            let Some(w) = &s.walk else {
                c.err(at, "a path needs `walk`");
                return c.fail(&s.id);
            };
            if w.path.len() < 2 {
                c.err(&at.key("walk"), "a path needs a `path` of two points");
                return c.fail(&s.id);
            }
            site::SiteForm::Walk(core::walk::Walk {
                path: w.path.iter().map(|p| c.p(*p)).collect(),
                width: c.l(w.width),
                surface: match w.surface {
                    f::PathSurface::Gravel => core::walk::Surface::Gravel,
                    f::PathSurface::Paving => core::walk::Surface::Paving,
                    f::PathSurface::Asphalt => core::walk::Surface::Asphalt,
                    f::PathSurface::Bark => core::walk::Surface::Bark,
                },
                edging: c.l(w.edging),
                thickness: c.l(w.thickness),
                radius: c.l(w.radius),
            })
        }
        f::SiteKind::Drain => {
            let Some(d) = &s.drain else {
                c.err(at, "a drain needs `drain`");
                return c.fail(&s.id);
            };
            if d.path.len() < 2 {
                c.err(&at.key("drain"), "a drain needs a `path` of two points");
                return c.fail(&s.id);
            }
            site::SiteForm::Drain(core::services::Drain {
                path: d.path.iter().map(|p| c.p(*p)).collect(),
                service: match d.service {
                    f::DrainService::Foul => core::services::DrainUse::Foul,
                    f::DrainService::Surface => core::services::DrainUse::Surface,
                    f::DrainService::Combined => core::services::DrainUse::Combined,
                    f::DrainService::Field => core::services::DrainUse::Field,
                },
                diameter: c.l(d.diameter),
                fall: d.fall,
                depth: c.l(d.depth),
                chambers: d.chambers,
                chamber: c.l(d.chamber),
                trench: c.l(d.trench),
                bed: c.l(d.bed),
            })
        }
        f::SiteKind::Investigation => {
            let (Some(p), Some(q)) = (s.at, &s.investigation) else {
                c.err(at, "an investigation point needs `at` and `investigation`");
                return c.fail(&s.id);
            };
            use core::investigation as inv;
            let text = |t: &Option<String>| t.clone().unwrap_or_default();
            site::SiteForm::Investigation(inv::Probe {
                position: c.p(p),
                kind: match q.kind {
                    f::ProbeType::Borehole => inv::ProbeKind::Borehole,
                    f::ProbeType::TrialPit => inv::ProbeKind::TrialPit,
                    f::ProbeType::Probe => inv::ProbeKind::Probe,
                },
                ground: q.ground.map(|g| c.l(g)),
                depth: c.l(q.depth),
                layers: q
                    .layers
                    .iter()
                    .map(|l| inv::LogLayer {
                        to: c.l(l.to),
                        soil: match l.soil {
                            f::SoilType::Topsoil => inv::SoilKind::Topsoil,
                            f::SoilType::MadeGround => inv::SoilKind::MadeGround,
                            f::SoilType::Clay => inv::SoilKind::Clay,
                            f::SoilType::Silt => inv::SoilKind::Silt,
                            f::SoilType::Sand => inv::SoilKind::Sand,
                            f::SoilType::Gravel => inv::SoilKind::Gravel,
                            f::SoilType::Peat => inv::SoilKind::Peat,
                            f::SoilType::Rock => inv::SoilKind::Rock,
                        },
                        unit: text(&l.unit),
                        description: text(&l.description),
                    })
                    .collect(),
                water: q
                    .water
                    .iter()
                    .map(|w| inv::WaterStrike {
                        depth: c.l(w.depth),
                        rest: w.rest.map(|r| c.l(r)),
                        note: text(&w.note),
                    })
                    .collect(),
                tests: q
                    .tests
                    .iter()
                    .map(|t| inv::FieldTest {
                        depth: c.l(t.depth),
                        kind: match t.kind {
                            f::TestType::Spt => inv::TestKind::Spt,
                            f::TestType::Vane => inv::TestKind::Vane,
                            f::TestType::Penetrometer => inv::TestKind::Penetrometer,
                            f::TestType::Dynamic => inv::TestKind::Dynamic,
                        },
                        value: t.value,
                        text: text(&t.text),
                    })
                    .collect(),
                method: text(&q.method),
                date: text(&q.date),
                report: text(&q.report),
                notes: text(&q.notes),
            })
        }
        f::SiteKind::Tank => {
            let (Some(p), Some(t)) = (s.at, &s.tank) else {
                c.err(at, "a tank needs `at` and `tank`");
                return c.fail(&s.id);
            };
            site::SiteForm::Tank(core::services::Tank {
                centre: c.p(p),
                service: match t.service {
                    f::TankService::Septic => core::services::TankUse::Septic,
                    f::TankService::Treatment => core::services::TankUse::Treatment,
                    f::TankService::Rainwater => core::services::TankUse::Rainwater,
                    f::TankService::Attenuation => core::services::TankUse::Attenuation,
                    f::TankService::Soakaway => core::services::TankUse::Soakaway,
                },
                length: c.l(t.length),
                width: c.l(t.width),
                depth: c.l(t.depth),
                angle: t.angle,
                cover: c.l(t.cover),
                wall: c.l(t.wall),
                working: c.l(t.working),
            })
        }
    };
    let el = core::Site {
        level,
        form,
        material,
    };
    let name = s.name.clone().unwrap_or_else(|| s.id.clone());
    c.insert(&s.id, Element::new(name, ElementKind::Site(el)), at);
}

fn pattern(c: &mut Ctx, p: &f::SitePattern, at: &Path) -> Option<site::Pattern> {
    let own: &[&str] = match p.kind {
        f::PatternKind::None => &["kind"],
        f::PatternKind::Units => &["kind", "size", "stagger", "angle"],
        f::PatternKind::Lines => &["kind", "spacing", "angle"],
        f::PatternKind::Stipple => &["kind", "spacing"],
    };
    let kind = format!("a {} pattern", p.kind.key());
    if !check_stray(c, p, own, &kind, at) {
        return None;
    }
    let spacing = |c: &mut Ctx| match p.spacing {
        Some(s) => Some(c.l(s)),
        None => {
            c.err(at, format!("{kind} needs `spacing`"));
            None
        }
    };
    Some(match p.kind {
        f::PatternKind::None => site::Pattern::None,
        f::PatternKind::Units => match p.size {
            Some(size) => site::Pattern::Units {
                size: c.p(size),
                stagger: p.stagger,
                angle: p.angle,
            },
            None => {
                c.err(at, "a units pattern needs `size`");
                return None;
            }
        },
        f::PatternKind::Lines => site::Pattern::Lines {
            spacing: spacing(c)?,
            angle: p.angle,
        },
        f::PatternKind::Stipple => site::Pattern::Stipple {
            spacing: spacing(c)?,
        },
    })
}

fn object(c: &mut Ctx, o: &f::Object, catalogue: &[CatalogueObject], at: &Path) {
    let Some(level) = c.get(&o.level, &[Tag::Level], &at.key("level")) else {
        return c.fail(&o.id);
    };
    let found = catalogue.iter().find(|x| x.key == o.catalogue).or_else(|| {
        catalogue
            .iter()
            .find(|x| x.key == format!("starter/{}", o.catalogue))
    });
    let Some(cat) = found else {
        let keys: Vec<&str> = catalogue.iter().map(|x| x.key.as_str()).collect();
        let short: Vec<&str> = keys
            .iter()
            .map(|k| k.trim_start_matches("starter/"))
            .collect();
        let hint = suggest(o.catalogue.trim_start_matches("starter/"), &short)
            .map(|s| format!("; did you mean \"starter/{s}\"?"))
            .unwrap_or_else(|| "; `creabim catalogue` lists them".into());
        c.err(
            &at.key("catalogue"),
            format!("no catalogue object \"{}\"{hint}", o.catalogue),
        );
        return c.fail(&o.id);
    };
    let at_plan = c.p(o.at);
    let position = match o.anchor {
        f::Anchor::Origin => at_plan,
        f::Anchor::Centre => core::objects::centred(&cat.object, at_plan, o.rotation),
    };
    let (mut tx, _) = core::objects::place(c.model(), cat, level, position, o.rotation);
    if !c.owns(&o.id, at) {
        return;
    }
    // its light belongs to its type: set on the type as it is made, or
    // on the one already there
    if let Some(l) = &o.light {
        let light = core::Light {
            on: l.on,
            lumens: l.lumens,
            kelvin: l.kelvin,
            beam: match l.beam {
                f::LightBeam::Narrow => core::Spread::Narrow,
                f::LightBeam::Medium => core::Spread::Medium,
                f::LightBeam::Wide => core::Spread::Wide,
                f::LightBeam::AllRound => core::Spread::AllRound,
            },
            group: l.group.clone().unwrap_or_default(),
        };
        let give = |e: &mut Element| -> bool {
            if let ElementKind::ElementType(t) = &mut e.kind {
                if let core::TypeParams::Object(ot) = &mut t.params {
                    ot.light = Some(light.clone());
                    return true;
                }
            }
            false
        };
        let mut set = false;
        for op in &mut tx.ops {
            if let core::Op::Insert(e) = op {
                set |= give(e);
            }
        }
        if !set {
            let there = core::objects::type_for_key(c.model(), &cat.key)
                .and_then(|id| c.model().get(id).cloned());
            if let Some(before) = there {
                let mut after = before.clone();
                if give(&mut after) && after != before {
                    tx.ops.push(core::Op::Replace { before, after });
                }
            }
        }
    }
    let eid = c.eid(&o.id);
    let k = c.k;
    for op in &mut tx.ops {
        if let core::Op::Insert(e) = op {
            if let ElementKind::Object(obj) = &mut e.kind {
                e.id = eid;
                if let Some(n) = &o.name {
                    e.name = n.clone();
                }
                e.mark = o.mark.clone();
                if let Some(off) = o.offset {
                    obj.offset = off * k;
                }
            }
        }
    }
    c.commit(&o.id, tx, at);
}

fn room(c: &mut Ctx, r: &f::Room, at: &Path) {
    let Some(level) = c.get(&r.level, &[Tag::Level], &at.key("level")) else {
        return c.fail(&r.id);
    };
    let mut boundary = c.pts(&r.outline);
    if boundary.len() >= 3 && core::polygon::signed_area2(&boundary) < 0.0 {
        boundary.reverse();
    }
    let room = core::Room {
        level,
        at: c.p(r.at),
        boundary,
        number: r.number.clone(),
        floor_finish: r.floor_finish.clone(),
        wall_finish: r.wall_finish.clone(),
        ceiling_finish: r.ceiling_finish.clone(),
        occupancy: r.occupancy.clone(),
        department: r.department.clone(),
        height: r.height.map(|h| c.l(h)),
    };
    c.insert(
        &r.id,
        Element::new(name_of(&r.name, &r.id), ElementKind::Room(room)),
        at,
    );
}

fn view(c: &mut Ctx, v: &f::View, at: &Path) {
    use f::ViewKind as K;
    const COMMON: &[&str] = &["id", "name", "kind", "scale", "style", "callout"];
    let own: &[&str] = match v.kind {
        K::Plan => &[
            "level",
            "cut_height",
            "ceiling_plan",
            "room_colours",
            "site",
        ],
        K::Section => &["start", "end", "depth", "room_colours", "site"],
        K::Elevation => &["start", "end", "depth", "site"],
        K::ThreeD => &["azimuth", "altitude", "site"],
        K::Schedule => &["category", "by_type"],
        K::LongSection => &["drain"],
        K::BoreholeLog => &["probe"],
        K::WalkSection => &["stair"],
    };
    let allowed: Vec<&str> = COMMON.iter().chain(own).copied().collect();
    let article = if v.kind == K::Elevation { "an" } else { "a" };
    let kind = format!("{article} {} view", v.kind.key());
    if !check_stray(c, v, &allowed, &kind, at) {
        return c.fail(&v.id);
    }
    let mut view = match v.kind {
        K::Plan => {
            let Some(l) = &v.level else {
                c.err(at, "a plan needs `level`");
                return c.fail(&v.id);
            };
            let Some(level) = c.get(l, &[Tag::Level], &at.key("level")) else {
                return c.fail(&v.id);
            };
            let mut view = core::View::plan(level);
            view.kind = core::ViewKind::Plan {
                level,
                cut_height: c.l(v.cut_height),
            };
            view.ceiling_plan = v.ceiling_plan;
            view
        }
        K::Section | K::Elevation => {
            let (Some(start), Some(end), Some(depth)) = (v.start, v.end, v.depth) else {
                c.err(at, format!("{kind} needs `start`, `end` and `depth`"));
                return c.fail(&v.id);
            };
            core::View::section(c.p(start), c.p(end), c.l(depth), v.kind == K::Section)
        }
        K::ThreeD => core::View::axonometric(v.azimuth, v.altitude),
        K::LongSection => {
            let Some(d) = &v.drain else {
                c.err(at, "a long section needs `drain`");
                return c.fail(&v.id);
            };
            let Some(drain) = c.get(d, &[Tag::Site], &at.key("drain")) else {
                return c.fail(&v.id);
            };
            core::View::long_section(drain)
        }
        K::BoreholeLog => {
            let Some(d) = &v.probe else {
                c.err(at, "a borehole log needs `probe`");
                return c.fail(&v.id);
            };
            let Some(probe) = c.get(d, &[Tag::Site], &at.key("probe")) else {
                return c.fail(&v.id);
            };
            core::View::borehole_log(probe)
        }
        K::WalkSection => {
            let Some(d) = &v.stair else {
                c.err(at, "a walk-line section needs `stair`");
                return c.fail(&v.id);
            };
            let Some(stair) = c.get(d, &[Tag::Stair], &at.key("stair")) else {
                return c.fail(&v.id);
            };
            core::View::walk_section(stair)
        }
        K::Schedule => {
            let Some(cat) = v.category else {
                c.err(at, "a schedule needs `category`");
                return c.fail(&v.id);
            };
            let category = match cat {
                f::ScheduleCategory::Doors => core::ScheduleCategory::Doors,
                f::ScheduleCategory::Windows => core::ScheduleCategory::Windows,
                f::ScheduleCategory::Walls => core::ScheduleCategory::Walls,
                f::ScheduleCategory::Slabs => core::ScheduleCategory::Slabs,
                f::ScheduleCategory::Structure => core::ScheduleCategory::Structure,
                f::ScheduleCategory::Furniture => core::ScheduleCategory::Furniture,
                f::ScheduleCategory::Fixtures => core::ScheduleCategory::Fixtures,
                f::ScheduleCategory::Rooms => core::ScheduleCategory::Rooms,
                f::ScheduleCategory::Drainage => core::ScheduleCategory::Drainage,
                f::ScheduleCategory::Stairs => core::ScheduleCategory::Stairs,
                f::ScheduleCategory::Parking => core::ScheduleCategory::Parking,
                f::ScheduleCategory::Materials => core::ScheduleCategory::Materials,
            };
            let mut view = core::View::schedule(category);
            view.kind = core::ViewKind::Schedule {
                category,
                by_type: v.by_type,
            };
            view
        }
    };
    if v.kind != K::Schedule {
        view.scale = v.scale;
    }
    if let Some(s) = v.style {
        view.style = match s {
            f::ViewStyle::HiddenLine => core::ViewStyle::HiddenLine,
            f::ViewStyle::Shaded => core::ViewStyle::Shaded,
            f::ViewStyle::Wireframe => core::ViewStyle::Wireframe,
        };
    }
    view.room_colours = v.room_colours;
    if let Some(sv) = &v.site {
        view.site = core::SiteView {
            look: None,
            setbacks: None,
            bearings: None,
            along: true,
            tints: true,
            tags: vec![],
            spot_tags: vec![],
            ground: match sv.ground {
                f::SiteGround::After => core::GroundState::After,
                f::SiteGround::Before => core::GroundState::Before,
                f::SiteGround::Both => core::GroundState::Both,
            },
            contours: sv.contours,
            levels: sv.levels,
            cut_fill: sv.cut_fill,
            banks: sv.banks,
            limit: sv.limit,
            legal: sv.legal,
            roads: sv.roads,
            services: sv.services,
            investigation: sv.investigation,
        };
    }
    if let Some(co) = &v.callout {
        let Some(parent) = c.get(&co.parent, &[Tag::View], &at.key("callout").key("parent")) else {
            return c.fail(&v.id);
        };
        view.callout = Some(core::element::Callout::new(
            parent,
            c.p(co.min),
            c.p(co.max),
        ));
    }
    c.insert(
        &v.id,
        Element::new(name_of(&v.name, &v.id), ElementKind::View(view)),
        at,
    );
}

fn sheet(c: &mut Ctx, s: &f::Sheet, at: &Path) {
    if !c.owns(&s.id, at) {
        return;
    }
    let mut sheet = match (s.size, s.width, s.height) {
        (Some(size), None, None) => core::Sheet::a(s.id.clone(), size.key()),
        (None, Some(w), Some(h)) => {
            let mut sheet = core::Sheet::a(s.id.clone(), "A3");
            sheet.width = w;
            sheet.height = h;
            sheet
        }
        (None, None, None) => {
            c.err(
                at,
                "a sheet needs `size` (\"A1\"...) or `width` and `height` in mm",
            );
            return c.fail(&s.id);
        }
        _ => {
            c.err(
                at,
                "give a sheet `size`, or `width` and `height`, not some of each",
            );
            return c.fail(&s.id);
        }
    };
    sheet.fields = s.fields.clone();
    let sheet_id = c.eid(&s.id);
    c.insert(
        &s.id,
        Element::new(name_of(&s.name, &s.id), ElementKind::Sheet(sheet)),
        at,
    );
    if c.broken.contains(&s.id) {
        return;
    }
    for (i, vp) in s.viewports.iter().enumerate() {
        let vat = at.key("viewports").index(i);
        let Some(view) = c.get(&vp.view, &[Tag::View], &vat.key("view")) else {
            continue;
        };
        let target = match (vp.target, &vp.crop) {
            (Some(t), _) => c.p(t),
            (None, Some(r)) => c.p((r.min + r.max) / 2.0),
            (None, None) => creabim_geom::section::view_drawing(c.model(), view)
                .and_then(|d| d.bounds())
                .map(|(lo, hi)| (lo + hi) / 2.0)
                .unwrap_or(DVec2::ZERO),
        };
        let viewport = core::Viewport {
            sheet: sheet_id,
            view,
            center: vp.at,
            target,
            crop: vp.crop.as_ref().map(|r| [c.p(r.min), c.p(r.max)]),
            fit: match &vp.fit {
                Some(f) => {
                    let Some(id) = c.get(f, &[Tag::Stair], &vat.key("fit")) else {
                        continue;
                    };
                    Some(id)
                }
                None => None,
            },
            pinned: false,
        };
        c.commit(
            &s.id,
            Transaction::new("Build")
                .insert(Element::new("Viewport", ElementKind::Viewport(viewport))),
            &vat,
        );
    }
}

/// A window's arrangement, as the file gives it.
fn arrangement(a: &f::Arrangement) -> creabim_core::Arrangement {
    creabim_core::Arrangement {
        kind: match a.kind {
            f::ArrangementKind::Ribbon => creabim_core::ArrangementKind::Ribbon,
            f::ArrangementKind::Corner => creabim_core::ArrangementKind::Corner,
            f::ArrangementKind::Bay => creabim_core::ArrangementKind::Bay,
            f::ArrangementKind::Bow => creabim_core::ArrangementKind::Bow,
        },
        group: a.group,
        corner_start: a.corner_start,
        corner_end: a.corner_end,
        left_start: a.left_start,
        left_end: a.left_end,
        angle_start: a.angle_start,
        angle_end: a.angle_end,
        joint: match a.joint {
            f::CornerJoint::Post => creabim_core::CornerJoint::Post,
            f::CornerJoint::Glass => creabim_core::CornerJoint::Glass,
        },
    }
}

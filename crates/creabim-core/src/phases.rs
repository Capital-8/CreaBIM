//! Phases: when each element is built and when it comes down.
//!
//! A project keeps an ordered list of phases ([`PhaseDef`], on its
//! [`crate::project::ProjectInfo`]); the usual three are Existing,
//! Demolition and Proposed. Every built thing remembers the phase it was
//! built in and the phase it is taken down in ([`Phasing`]). Whether it is
//! existing, to take down or new is never stored: it follows from the
//! phase a view stands at ([`status`]).
//!
//! A project with no phases is not phased: everything is always there.

use crate::{Element, ElementId, ElementKind, Model, Op, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// One phase of the project. Its place in the project's list is its place
/// in time; its id never changes, so elements and views keep pointing at
/// it when phases are renamed or reordered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseDef {
    pub id: u32,
    pub name: String,
    /// A phase things are taken down in: a view standing at it shows what
    /// comes down in it. A view standing at any other phase leaves out
    /// what came down in that phase.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub demolition: bool,
}

/// When an element is there: from the phase it is built in until the
/// phase it is taken down in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Phasing {
    /// The phase it is built in; none is the project's first.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub built: Option<u32>,
    /// The phase it is taken down in; none is never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone: Option<u32>,
    /// The option it is built in alone (see [`Question`]); none is shared
    /// by every option.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub option: Option<u32>,
    /// The option that alone takes it down; none is all of them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gone_option: Option<u32>,
}

impl Phasing {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// How the project draws each status, in every view and in 3D: the
/// office's own convention for existing, demolished and new work.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PhaseLook {
    /// The colour of what a view's phase takes down.
    pub down: [f32; 3],
    /// Whether what comes down is drawn in dashes.
    pub down_dashed: bool,
    /// Whether what was there before is drawn fainter and unhatched.
    pub existing_faint: bool,
    /// A colour for what a view's phase builds; none keeps its own.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<[f32; 3]>,
}

/// The red what comes down is drawn in unless the project says otherwise.
pub const DOWN_RED: [f32; 3] = [0.76, 0.27, 0.16];

impl Default for PhaseLook {
    fn default() -> Self {
        Self {
            down: DOWN_RED,
            down_dashed: true,
            existing_faint: true,
            new: None,
        }
    }
}

impl PhaseLook {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// How the project draws each status.
pub fn look(model: &Model) -> PhaseLook {
    model
        .project_info()
        .map(|(_, p)| p.phase_look)
        .unwrap_or_default()
}

/// Set how the project draws each status, through `f`.
pub fn set_look(model: &Model, f: impl FnOnce(&mut PhaseLook)) -> Option<Transaction> {
    let (id, _) = model.project_info()?;
    let before = model.get(id)?.clone();
    let mut after = before.clone();
    if let ElementKind::Project(p) = &mut after.kind {
        f(&mut p.phase_look);
    }
    (after != before).then(|| Transaction::new("How phases draw").replace(before, after))
}

/// How an element reads from the phase a view stands at.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    /// Not built yet, or already gone.
    Absent,
    /// Built in an earlier phase and still standing.
    Existing,
    /// Taken down in this phase.
    TakenDown,
    /// Built in this phase.
    New,
    /// Of an option the view does not show but ghosts: drawn faintly, to
    /// compare with or trace over.
    Ghost,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Absent => "not here",
            Status::Existing => "existing",
            Status::TakenDown => "to take down",
            Status::New => "new",
            Status::Ghost => "another option, ghosted",
        }
    }
}

/// The three phases a project starts with.
pub fn standard() -> Vec<PhaseDef> {
    vec![
        PhaseDef {
            id: 1,
            name: "Existing".into(),
            demolition: false,
        },
        PhaseDef {
            id: 2,
            name: "Demolition".into(),
            demolition: true,
        },
        PhaseDef {
            id: 3,
            name: "Proposed".into(),
            demolition: false,
        },
    ]
}

/// The project's phases in order; empty for a project that has none.
pub fn list(model: &Model) -> &[PhaseDef] {
    model
        .project_info()
        .map(|(_, p)| p.phases.as_slice())
        .unwrap_or(&[])
}

/// Where phase `id` is in `list`.
pub fn position(list: &[PhaseDef], id: u32) -> Option<usize> {
    list.iter().position(|p| p.id == id)
}

/// The phase a view (or the 3D window) set to `at` stands at: the one it
/// names, else the project's last.
pub fn stands_at(list: &[PhaseDef], at: Option<u32>) -> usize {
    at.and_then(|id| position(list, id))
        .unwrap_or(list.len().saturating_sub(1))
}

/// The phase a view set to `at` stands at, by id; none for a project
/// without phases.
pub fn stands_at_id(list: &[PhaseDef], at: Option<u32>) -> Option<u32> {
    list.get(stands_at(list, at)).map(|p| p.id)
}

/// Is `e` something built, which phases apply to? Annotations, views,
/// levels, grids and the project's own settings are not.
pub fn phased(e: &Element) -> bool {
    matches!(
        e.kind,
        ElementKind::Wall(_)
            | ElementKind::Slab(_)
            | ElementKind::Opening(_)
            | ElementKind::FreeMesh(_)
            | ElementKind::Column(_)
            | ElementKind::Beam(_)
            | ElementKind::Roof(_)
            | ElementKind::Stair(_)
            | ElementKind::Railing(_)
            | ElementKind::Shaft(_)
            | ElementKind::Object(_)
            | ElementKind::Footing(_)
            | ElementKind::Part(_)
            | ElementKind::Site(_)
            | ElementKind::Room(_)
            | ElementKind::RoomSeparator(_)
            | ElementKind::Ceiling(_)
    )
}

/// How something with phasing `p` reads from the phase at place `at` of
/// `list`.
pub fn status(list: &[PhaseDef], p: Phasing, at: usize) -> Status {
    if list.is_empty() {
        return Status::New;
    }
    let built = p.built.and_then(|id| position(list, id)).unwrap_or(0);
    if built > at {
        return Status::Absent;
    }
    if let Some(gone) = p.gone.and_then(|id| position(list, id)) {
        if gone < at || gone < built {
            return Status::Absent;
        }
        if gone == at {
            return if list[at].demolition {
                Status::TakenDown
            } else {
                Status::Absent
            };
        }
    }
    if built == at {
        Status::New
    } else {
        Status::Existing
    }
}

/// How each built element of `model` reads from the phase `at` names
/// ([`stands_at`]). Empty for a project without phases.
pub fn statuses(
    model: &Model,
    at: Option<u32>,
    options: &[u32],
    ghost: &[u32],
) -> HashMap<ElementId, Status> {
    let list = list(model);
    if list.is_empty() {
        return HashMap::new();
    }
    let at = stands_at(list, at);
    let chosen = chosen(model, options);
    model
        .iter()
        .filter(|e| phased(e))
        .map(|e| (e.id, reads_ghosted(list, e.phase, at, &chosen, ghost)))
        .collect()
}

/// What is not there at the phase `at` names: the built elements absent
/// from it, and everything that hangs on them (an absent wall's doors, the
/// tags and dimensions of what is absent).
pub fn absent(
    model: &Model,
    at: Option<u32>,
    options: &[u32],
    ghost: &[u32],
) -> HashSet<ElementId> {
    let list = list(model);
    let mut out = HashSet::new();
    if list.is_empty() {
        return out;
    }
    let at = stands_at(list, at);
    let chosen = chosen(model, options);
    for e in model.iter() {
        if phased(e) && reads_ghosted(list, e.phase, at, &chosen, ghost) == Status::Absent {
            out.insert(e.id);
        }
    }
    if out.is_empty() {
        return out;
    }
    // what refers to something absent goes with it, and so on outward
    let refs: Vec<(ElementId, Vec<ElementId>)> = model
        .iter()
        .filter(|e| !out.contains(&e.id) && !keeps_without(e))
        .map(|e| (e.id, e.references()))
        .filter(|(_, r)| !r.is_empty())
        .collect();
    loop {
        let before = out.len();
        for (id, r) in &refs {
            if !out.contains(id) && r.iter().any(|x| out.contains(x)) {
                out.insert(*id);
            }
        }
        if out.len() == before {
            break;
        }
    }
    out
}

/// Stays in a phase's model even when something it refers to is absent
/// from it: views, sheets and the project's own settings.
fn keeps_without(e: &Element) -> bool {
    matches!(
        e.kind,
        ElementKind::View(_)
            | ElementKind::Sheet(_)
            | ElementKind::Viewport(_)
            | ElementKind::Level(_)
            | ElementKind::Material(_)
            | ElementKind::Project(_)
            | ElementKind::StylePack(_)
    )
}

/// The phase models made lately: (the model's revision, the phase's
/// place), and the model.
type AtCache = Vec<((u64, usize, Vec<u32>, Vec<u32>), Arc<Model>)>;
static AT: Mutex<AtCache> = Mutex::new(Vec::new());

/// `model` as it is at the phase `at` names: without what is not there
/// then ([`absent`]). A wall keeps no hole for a door that is not there
/// yet, and has none left by a door that is gone. `None` when nothing is
/// absent, so the caller draws from `model` itself.
///
/// The copy is kept per revision and phase, so every view standing at the
/// same phase draws from the same model, and shares what is built from it
/// by its revision. It may refer to elements it left out, so it is for
/// reading only.
pub fn model_at(
    model: &Model,
    at: Option<u32>,
    options: &[u32],
    ghost: &[u32],
) -> Option<Arc<Model>> {
    let list = list(model);
    if list.is_empty() {
        return None;
    }
    let key = (
        model.revision(),
        stands_at(list, at),
        chosen(model, options),
        ghost.to_vec(),
    );
    if key.0 != 0 {
        let cache = AT.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, m)) = cache.iter().find(|(k, _)| *k == key) {
            return Some(m.clone());
        }
    }
    let gone = absent(model, at, options, ghost);
    if gone.is_empty() {
        return None;
    }
    let mut m = model.clone();
    m.retain(|e| !gone.contains(&e.id));
    let m = Arc::new(m);
    if key.0 != 0 {
        let mut cache = AT.lock().unwrap_or_else(|e| e.into_inner());
        if cache.len() >= 8 {
            cache.remove(0);
        }
        cache.push((key, m.clone()));
    }
    Some(m)
}

/// Give the project its phases: the standard three, with what is drawn so
/// far as the existing building (`drawn_is_existing`) or as the proposed
/// one. Every view is set to stand at the phase the drawing belongs to,
/// so nothing changes on screen until a view is moved through time.
/// `None` when the project has phases already.
pub fn start(model: &Model, drawn_is_existing: bool) -> Option<Transaction> {
    if !list(model).is_empty() {
        return None;
    }
    let phases = standard();
    let home = if drawn_is_existing {
        phases[0].id
    } else {
        phases[2].id
    };
    let mut tx = Transaction::new("Start phases");
    match model.project_info() {
        Some((id, _)) => {
            let before = model.get(id)?.clone();
            let mut after = before.clone();
            if let ElementKind::Project(p) = &mut after.kind {
                p.phases = phases;
            }
            tx = tx.replace(before, after);
        }
        None => {
            let info = crate::project::ProjectInfo {
                phases,
                ..Default::default()
            };
            tx = tx.insert(Element::new("Project", ElementKind::Project(info)));
        }
    }
    for e in model.iter() {
        let mut after = e.clone();
        match &mut after.kind {
            ElementKind::View(v) => v.phase = Some(home),
            _ if phased(e) && !drawn_is_existing => after.phase.built = Some(home),
            _ => continue,
        }
        if after != *e {
            tx = tx.replace(e.clone(), after);
        }
    }
    Some(tx)
}

/// Set when `ids` are built and taken down, through `f`. A phase that
/// would leave an element gone before it is built is refused: the element
/// is left as it was.
pub fn set(model: &Model, ids: &[ElementId], label: &str, f: impl Fn(&mut Phasing)) -> Transaction {
    let list = list(model);
    let mut tx = Transaction::new(label);
    for id in ids {
        let Some(e) = model.get(*id).filter(|e| phased(e)) else {
            continue;
        };
        let mut after = e.clone();
        f(&mut after.phase);
        let built = after
            .phase
            .built
            .and_then(|b| position(list, b))
            .unwrap_or(0);
        if after
            .phase
            .built
            .is_some_and(|b| position(list, b) == Some(0))
        {
            after.phase.built = None;
        }
        let gone = after.phase.gone.and_then(|g| position(list, g));
        if after.phase.gone.is_some() && gone.is_none_or(|g| g < built) {
            continue;
        }
        if after != *e {
            tx = tx.replace(e.clone(), after);
        }
    }
    tx
}

/// The phase something built at place `built` is taken down in when it
/// is removed from the phase at place `at`: the latest demolition phase
/// after it was built and not after `at`, else `at` itself.
pub fn comes_down_in(list: &[PhaseDef], built: usize, at: usize) -> Option<u32> {
    (built + 1..=at)
        .rev()
        .find(|i| list.get(*i).is_some_and(|p| p.demolition))
        .or(Some(at))
        .and_then(|i| list.get(i))
        .map(|p| p.id)
}

/// Remove `ids` from the phase `at` names onward. What was built in that
/// phase is deleted outright. What was built earlier is taken down: it
/// stays in the earlier phases and is gone from this one. What hangs on
/// something taken down (a wall's doors and windows) comes down with it.
/// The second value is how many were taken down, not deleted.
pub fn remove_from(model: &Model, ids: &[ElementId], at: Option<u32>) -> (Transaction, usize) {
    remove_from_option(model, ids, at, None)
}

/// [`remove_from`] while working in option `option`: what the option
/// itself holds is removed as there; what is shared is taken down by this
/// option alone, and stays in the others.
pub fn remove_from_option(
    model: &Model,
    ids: &[ElementId],
    at: Option<u32>,
    option: Option<u32>,
) -> (Transaction, usize) {
    let list = list(model);
    let mut tx = Transaction::new("Delete");
    if list.is_empty() {
        return (tx.remove_all_cascade(model, ids), 0);
    }
    let at = stands_at(list, at);
    let mut delete = vec![];
    let mut down: Vec<ElementId> = vec![];
    for id in ids {
        let Some(e) = model.get(*id) else { continue };
        let built = e.phase.built.and_then(|b| position(list, b)).unwrap_or(0);
        // shared, and removed from one option only: it stays in the others
        let others_keep = option.is_some() && e.phase.option.is_none();
        if phased(e) && (built < at || others_keep) {
            down.push(*id);
        } else {
            delete.push(*id);
        }
    }
    // doors and windows of a wall taken down come down with it
    let mut i = 0;
    while i < down.len() {
        for d in model.dependents(down[i]) {
            let hosted = matches!(model.get(d).map(|e| &e.kind), Some(ElementKind::Opening(_)));
            if hosted && !down.contains(&d) {
                down.push(d);
            }
        }
        i += 1;
    }
    let mut taken = 0;
    for id in &down {
        let Some(e) = model.get(*id) else { continue };
        if e.phase
            .gone
            .and_then(|g| position(list, g))
            .is_some_and(|g| g <= at)
        {
            continue;
        }
        let built = e.phase.built.and_then(|b| position(list, b)).unwrap_or(0);
        let others_keep = option.is_some() && e.phase.option.is_none();
        if built >= at && !others_keep {
            delete.push(*id);
            continue;
        }
        let mut after = e.clone();
        after.phase.gone = if built >= at {
            list.get(at).map(|p| p.id)
        } else {
            comes_down_in(list, built, at)
        };
        after.phase.gone_option = if others_keep { option } else { None };
        tx = tx.replace(e.clone(), after);
        taken += 1;
    }
    delete.retain(|id| {
        !down.contains(id)
            || model.get(*id).is_some_and(|e| {
                e.phase.built.and_then(|b| position(list, b)).unwrap_or(0) >= at
                    && !(option.is_some() && e.phase.option.is_none())
            })
    });
    (tx.remove_all_cascade(model, &delete), taken)
}

/// Mark what `tx` adds as built in phase `working`: what is drawn belongs
/// to the phase being worked in. A change that also removes something of
/// the same kind (a wall split in two, an element swapped for another) is
/// left alone, so the pieces keep the phase of what they came from.
pub fn stamp(model: &Model, tx: &mut Transaction, working: u32, option: Option<u32>) {
    let list = list(model);
    let Some(at) = position(list, working) else {
        return;
    };
    let built = (at > 0).then_some(working);
    let removed: HashSet<std::mem::Discriminant<ElementKind>> = tx
        .ops
        .iter()
        .filter_map(|o| match o {
            Op::Remove(e) if phased(e) => Some(std::mem::discriminant(&e.kind)),
            _ => None,
        })
        .collect();
    for op in &mut tx.ops {
        let Op::Insert(e) = op else { continue };
        if let ElementKind::View(v) = &mut e.kind {
            if v.phase.is_none() {
                v.phase = Some(working);
            }
        } else if phased(e) && !removed.contains(&std::mem::discriminant(&e.kind)) {
            e.phase = Phasing {
                built,
                gone: None,
                option,
                gone_option: None,
            };
        }
    }
}

/// One way of answering a [`Question`]: "Option A".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OptionDef {
    pub id: u32,
    pub name: String,
}

/// A question the design has open, with the options that answer it
/// ("Extension": deep, or full width). An option holds only what differs:
/// what is built in it alone, and what it alone takes down. Everything
/// else is shared by all of them. Ids are unique among the project's
/// questions and options together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    pub id: u32,
    pub name: String,
    pub options: Vec<OptionDef>,
    /// The option views show unless they say otherwise.
    pub pick: u32,
}

/// The project's open questions.
pub fn questions(model: &Model) -> &[Question] {
    model
        .project_info()
        .map(|(_, p)| p.questions.as_slice())
        .unwrap_or(&[])
}

/// The option shown for each question: the one of `choices` that answers
/// it, else its pick.
pub fn chosen(model: &Model, choices: &[u32]) -> Vec<u32> {
    questions(model)
        .iter()
        .map(|q| {
            q.options
                .iter()
                .map(|o| o.id)
                .find(|o| choices.contains(o))
                .unwrap_or(q.pick)
        })
        .collect()
}

/// The question option `id` answers, and the option.
pub fn option(model: &Model, id: u32) -> Option<(&Question, &OptionDef)> {
    questions(model)
        .iter()
        .find_map(|q| q.options.iter().find(|o| o.id == id).map(|o| (q, o)))
}

/// `p` as it is with the options `chosen` shown: none when it is built in
/// an option that is not shown; not taken down when only an option that
/// is not shown takes it down.
pub fn effective(p: Phasing, chosen: &[u32]) -> Option<Phasing> {
    if p.option.is_some_and(|o| !chosen.contains(&o)) {
        return None;
    }
    let mut p = p;
    if p.gone_option.is_some_and(|o| !chosen.contains(&o)) {
        p.gone = None;
    }
    Some(p)
}

/// [`reads`], with what the options `ghost` hold read as [`Status::Ghost`]
/// where the options shown leave it out.
pub fn reads_ghosted(
    list: &[PhaseDef],
    p: Phasing,
    at: usize,
    chosen: &[u32],
    ghost: &[u32],
) -> Status {
    let s = reads(list, p, at, chosen);
    if s != Status::Absent || ghost.is_empty() {
        return s;
    }
    match p.option {
        Some(o) if ghost.contains(&o) => {
            let mut with = chosen.to_vec();
            with.push(o);
            match reads(list, p, at, &with) {
                Status::Absent => Status::Absent,
                _ => Status::Ghost,
            }
        }
        _ => Status::Absent,
    }
}

/// Turn the changes `tx` makes to existing work into alterations: the
/// element as it was comes down, and the changed one is built in phase
/// `working` (in option `option` alone, when one is worked in).
///
/// The changed element keeps its id, so a drag that goes on changing it,
/// and its tags and dimensions, stay with it; the element as it was is
/// kept as a copy, taken down. Its doors and windows go the same way: the
/// ones in the changed wall are built with it, and copies stay in the old
/// one. Only plain edits are altered: a transaction that adds or removes
/// built things (a split, a swap) is left as it is.
pub fn alter(model: &Model, tx: &mut Transaction, working: u32, option: Option<u32>) -> usize {
    let list = list(model);
    let Some(at) = position(list, working) else {
        return 0;
    };
    if at == 0 || tx.ops.iter().any(|o| !matches!(o, Op::Replace { .. })) {
        return 0;
    }
    let place = |p: Option<u32>| p.and_then(|p| position(list, p)).unwrap_or(0);
    let mut extra: Vec<Op> = vec![];
    let mut altered = 0;
    let touched: HashSet<ElementId> = tx
        .ops
        .iter()
        .filter_map(|o| match o {
            Op::Replace { before, .. } => Some(before.id),
            _ => None,
        })
        .collect();
    for op in &mut tx.ops {
        let Op::Replace { before, after } = op else {
            continue;
        };
        // existing work, standing, changed in more than its phases or name
        if !phased(before) || before.phase.gone.is_some() || place(before.phase.built) >= at {
            continue;
        }
        if matches!(before.kind, ElementKind::Opening(_)) {
            // a door or window moved in a wall that stays: itself altered
            let host_altered = match &before.kind {
                ElementKind::Opening(o) => touched.contains(&o.host),
                _ => false,
            };
            if host_altered {
                continue;
            }
        }
        let mut same = before.clone();
        same.phase = after.phase;
        same.name = after.name.clone();
        same.mark = after.mark.clone();
        if same == *after {
            continue;
        }
        let built = place(before.phase.built);
        let down = comes_down_in(list, built, at);
        // as it was: a copy, taken down
        let mut old = before.clone();
        old.id = ElementId::new();
        old.phase.gone = down;
        old.phase.gone_option = if before.phase.option.is_none() {
            option
        } else {
            None
        };
        // what is in it: built with the changed one; copies stay in the old
        for d in model.dependents(before.id) {
            let Some(o) = model
                .get(d)
                .filter(|x| matches!(x.kind, ElementKind::Opening(_)))
            else {
                continue;
            };
            if touched.contains(&o.id) || o.phase.gone.is_some() {
                continue;
            }
            let mut kept = o.clone();
            kept.id = ElementId::new();
            if let ElementKind::Opening(k) = &mut kept.kind {
                k.host = old.id;
            }
            kept.phase.gone = down;
            kept.phase.gone_option = old.phase.gone_option;
            let mut moved = o.clone();
            if place(moved.phase.built) < at {
                moved.phase.built = Some(working);
                moved.phase.option = moved.phase.option.or(option);
            }
            extra.push(Op::Insert(kept));
            if moved != *o {
                extra.push(Op::Replace {
                    before: o.clone(),
                    after: moved,
                });
            }
        }
        extra.insert(0, Op::Insert(old));
        after.phase.built = Some(working);
        after.phase.option = after.phase.option.or(option);
        altered += 1;
    }
    // the old copies go in first, so what refers to them finds them
    extra.sort_by_key(|o| match o {
        Op::Insert(e) if !matches!(e.kind, ElementKind::Opening(_)) => 0,
        Op::Insert(_) => 1,
        _ => 2,
    });
    tx.ops.extend(extra);
    altered
}

/// The doors and windows taken down from walls that stay: where a wall
/// has to be made good. Each with its wall and the area to fill, m².
pub fn making_good(model: &Model) -> Vec<(ElementId, ElementId, f64)> {
    model
        .iter()
        .filter_map(|e| {
            let ElementKind::Opening(o) = &e.kind else {
                return None;
            };
            let gone = e.phase.gone?;
            let host = model.get(o.host)?;
            // the wall comes down with it: nothing to make good
            if host.phase.gone == Some(gone) {
                return None;
            }
            Some((e.id, host.id, o.width * o.height))
        })
        .collect()
}

/// What a change is refused with while shared parts are held.
pub const HELD: &str = "Held: this is shared by all options. Turn off Hold shared in the Phases tab, or work in what all options share.";

/// The first element of `tx` that is shared by all options and that `tx`
/// changes or removes: what holding the shared parts refuses. A change of
/// when it is built or taken down alone is let through, so an option can
/// still take a shared thing down.
pub fn held_change(tx: &Transaction) -> Option<ElementId> {
    tx.ops.iter().find_map(|op| match op {
        Op::Replace { before, after }
            if phased(before) && before.phase.option.is_none() && after.phase.option.is_none() =>
        {
            let mut as_after = before.clone();
            as_after.phase = after.phase;
            (as_after != *after).then_some(before.id)
        }
        Op::Remove(e) if phased(e) && e.phase.option.is_none() => Some(e.id),
        _ => None,
    })
}

/// Everything taken down, in any phase or option: what [`bring_back`]
/// can restore.
pub fn taken_down(model: &Model) -> Vec<ElementId> {
    model
        .iter()
        .filter(|e| phased(e) && e.phase.gone.is_some())
        .map(|e| e.id)
        .collect()
}

/// Undo the taking down of `ids`: they stand again from the phase they
/// were built in, in every option. What came down with one of them (a
/// wall's doors and windows, taken down in the same phase) comes back
/// with it.
pub fn bring_back(model: &Model, ids: &[ElementId]) -> Transaction {
    let mut all: Vec<ElementId> = vec![];
    for id in ids {
        let Some(e) = model.get(*id).filter(|e| e.phase.gone.is_some()) else {
            continue;
        };
        all.push(*id);
        for d in model.dependents(*id) {
            let with = model.get(d).is_some_and(|x| {
                matches!(x.kind, ElementKind::Opening(_)) && x.phase.gone == e.phase.gone
            });
            if with {
                all.push(d);
            }
        }
    }
    all.dedup();
    let mut tx = Transaction::new("Bring back");
    let mut done = HashSet::new();
    for id in all {
        let Some(e) = model.get(id).filter(|_| done.insert(id)) else {
            continue;
        };
        let mut after = e.clone();
        after.phase.gone = None;
        after.phase.gone_option = None;
        if after != *e {
            tx = tx.replace(e.clone(), after);
        }
    }
    tx
}

/// The questions settled so far, kept so they can be opened again.
pub fn shelf(model: &Model) -> &[Question] {
    model
        .project_info()
        .map(|(_, p)| p.shelf.as_slice())
        .unwrap_or(&[])
}

/// Open settled question `id` again, with its options and what they
/// still hold. What the option it was settled on built has become the
/// project's own and stays shared.
pub fn reopen(model: &Model, id: u32) -> Option<Transaction> {
    let (pid, _) = model.project_info()?;
    let before = model.get(pid)?.clone();
    let mut after = before.clone();
    if let ElementKind::Project(p) = &mut after.kind {
        let at = p.shelf.iter().position(|q| q.id == id)?;
        let q = p.shelf.remove(at);
        p.questions.push(q);
    }
    Some(Transaction::new("Reopen question").replace(before, after))
}

/// [`status`] with the options `chosen` shown.
pub fn reads(list: &[PhaseDef], p: Phasing, at: usize, chosen: &[u32]) -> Status {
    match effective(p, chosen) {
        Some(p) => status(list, p, at),
        None => Status::Absent,
    }
}

fn edit_questions(
    model: &Model,
    label: &str,
    f: impl FnOnce(&mut Vec<Question>),
) -> Option<Transaction> {
    let (id, _) = model.project_info()?;
    let before = model.get(id)?.clone();
    let mut after = before.clone();
    if let ElementKind::Project(p) = &mut after.kind {
        f(&mut p.questions);
    }
    (after != before).then(|| Transaction::new(label).replace(before, after))
}

fn next_option_id(model: &Model) -> u32 {
    questions(model)
        .iter()
        .chain(shelf(model))
        .flat_map(|q| std::iter::once(q.id).chain(q.options.iter().map(|o| o.id)))
        .max()
        .unwrap_or(0)
        + 1
}

/// Open a question named `name` with two options, A and B, A its pick.
/// The second value is the two options. `None` for a project without
/// phases.
pub fn add_question(model: &Model, name: &str) -> Option<(Transaction, [u32; 2])> {
    if list(model).is_empty() {
        return None;
    }
    let id = next_option_id(model);
    let (a, b) = (id + 1, id + 2);
    let name = if name.trim().is_empty() {
        "Question"
    } else {
        name.trim()
    }
    .to_string();
    let q = Question {
        id,
        name,
        options: vec![
            OptionDef {
                id: a,
                name: "Option A".into(),
            },
            OptionDef {
                id: b,
                name: "Option B".into(),
            },
        ],
        pick: a,
    };
    Some((
        edit_questions(model, "Add question", |l| l.push(q))?,
        [a, b],
    ))
}

/// Another option for question `q`, lettered after the ones it has.
pub fn add_option(model: &Model, q: u32) -> Option<(Transaction, u32)> {
    let id = next_option_id(model);
    let n = questions(model).iter().find(|x| x.id == q)?.options.len();
    let name = format!("Option {}", (b'A' + (n as u8).min(25)) as char);
    let tx = edit_questions(model, "Add option", |l| {
        if let Some(x) = l.iter_mut().find(|x| x.id == q) {
            x.options.push(OptionDef { id, name });
        }
    })?;
    Some((tx, id))
}

/// Name question or option `id` `name`.
pub fn rename_option(model: &Model, id: u32, name: &str) -> Option<Transaction> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    edit_questions(model, "Rename option", |l| {
        for q in l.iter_mut() {
            if q.id == id {
                q.name = name.to_string();
            }
            for o in &mut q.options {
                if o.id == id {
                    o.name = name.to_string();
                }
            }
        }
    })
}

/// Make option `id` its question's pick: what views show unless they say
/// otherwise.
pub fn set_pick(model: &Model, id: u32) -> Option<Transaction> {
    edit_questions(model, "Pick option", |l| {
        for q in l.iter_mut() {
            if q.options.iter().any(|o| o.id == id) {
                q.pick = id;
            }
        }
    })
}

/// Close the question option `id` answers, with that option: what it
/// builds and takes down becomes the project's own, shared. What the
/// other options hold is kept in the file, out of every view, and the
/// second value says how many elements that is.
pub fn settle(model: &Model, id: u32) -> Option<(Transaction, usize)> {
    let (q, _) = option(model, id)?;
    let (qid, others): (u32, Vec<u32>) = (
        q.id,
        q.options
            .iter()
            .map(|o| o.id)
            .filter(|o| *o != id)
            .collect(),
    );
    let mut kept = q.clone();
    kept.pick = id;
    let (pid, _) = model.project_info()?;
    let before = model.get(pid)?.clone();
    let mut after = before.clone();
    if let ElementKind::Project(p) = &mut after.kind {
        p.questions.retain(|x| x.id != qid);
        p.shelf.push(kept);
    }
    let mut tx = Transaction::new("Settle question").replace(before, after);
    let mut shelved = 0;
    for e in model.iter().filter(|e| phased(e)) {
        let mut after = e.clone();
        if after.phase.option == Some(id) {
            after.phase.option = None;
        }
        if after.phase.gone_option == Some(id) {
            after.phase.gone_option = None;
        }
        // what only another option took down stays up
        if after.phase.gone_option.is_some_and(|o| others.contains(&o)) {
            after.phase.gone = None;
            after.phase.gone_option = None;
        }
        if e.phase.option.is_some_and(|o| others.contains(&o)) {
            shelved += 1;
        }
        if after != *e {
            tx = tx.replace(e.clone(), after);
        }
    }
    Some((tx, shelved))
}

/// Put `ids` in option `to`, or make them shared (none).
pub fn move_to_option(model: &Model, ids: &[ElementId], to: Option<u32>) -> Transaction {
    let mut tx = Transaction::new(if to.is_some() {
        "Move to option"
    } else {
        "Make shared"
    });
    for id in ids {
        let Some(e) = model.get(*id).filter(|e| phased(e)) else {
            continue;
        };
        let mut after = e.clone();
        after.phase.option = to;
        if after != *e {
            tx = tx.replace(e.clone(), after);
        }
    }
    tx
}

/// How many built elements option `id` holds: built in it alone, and
/// taken down by it alone.
pub fn option_uses(model: &Model, id: u32) -> (usize, usize) {
    let (mut built, mut down) = (0, 0);
    for e in model.iter().filter(|e| phased(e)) {
        if e.phase.option == Some(id) {
            built += 1;
        }
        if e.phase.gone_option == Some(id) {
            down += 1;
        }
    }
    (built, down)
}

/// Change the project's list of phases through `f`, as one edit. `None`
/// when the project has none, or `f` changes nothing.
fn edit_list(
    model: &Model,
    label: &str,
    f: impl FnOnce(&mut Vec<PhaseDef>),
) -> Option<Transaction> {
    let (id, _) = model.project_info()?;
    let before = model.get(id)?.clone();
    let mut after = before.clone();
    if let ElementKind::Project(p) = &mut after.kind {
        if p.phases.is_empty() {
            return None;
        }
        f(&mut p.phases);
    }
    (after != before).then(|| Transaction::new(label).replace(before, after))
}

/// How many built elements are built in phase `id`, and how many are
/// taken down in it.
pub fn uses(model: &Model, id: u32) -> (usize, usize) {
    let list = list(model);
    let first = position(list, id) == Some(0);
    let (mut built, mut down) = (0, 0);
    for e in model.iter().filter(|e| phased(e)) {
        if e.phase.built == Some(id) || (first && e.phase.built.is_none()) {
            built += 1;
        }
        if e.phase.gone == Some(id) {
            down += 1;
        }
    }
    (built, down)
}

/// Add a phase named `name` after phase `after` (at the end when none),
/// and its id. Its name is made unlike the others' with a number.
pub fn add(model: &Model, name: &str, after: Option<u32>) -> Option<(Transaction, u32)> {
    let list = list(model);
    let id = list.iter().map(|p| p.id).max()? + 1;
    let mut name = name.trim().to_string();
    if name.is_empty() {
        name = "Phase".into();
    }
    let base = name.clone();
    let mut n = 2;
    while list.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
        name = format!("{base} {n}");
        n += 1;
    }
    let at = after
        .and_then(|a| position(list, a))
        .map_or(list.len(), |i| i + 1);
    let tx = edit_list(model, "Add phase", |l| {
        l.insert(
            at,
            PhaseDef {
                id,
                name,
                demolition: false,
            },
        )
    })?;
    Some((tx, id))
}

/// Name phase `id` `name`; an empty name, or another phase's, is refused.
pub fn rename(model: &Model, id: u32, name: &str) -> Option<Transaction> {
    let name = name.trim();
    if name.is_empty()
        || list(model)
            .iter()
            .any(|p| p.id != id && p.name.eq_ignore_ascii_case(name))
    {
        return None;
    }
    edit_list(model, "Rename phase", |l| {
        if let Some(p) = l.iter_mut().find(|p| p.id == id) {
            p.name = name.to_string();
        }
    })
}

/// Make phase `id` one that things are taken down in, or not.
pub fn set_demolition(model: &Model, id: u32, on: bool) -> Option<Transaction> {
    edit_list(model, "Phase takes things down", |l| {
        if let Some(p) = l.iter_mut().find(|p| p.id == id) {
            p.demolition = on;
        }
    })
}

/// Move phase `id` one place earlier (`up`) or later in time. Refused,
/// with why, when something would then be taken down before it is built.
pub fn shift(model: &Model, id: u32, up: bool) -> Result<Transaction, String> {
    let now = list(model);
    let at = position(now, id).ok_or("No such phase.")?;
    let to = if up {
        at.checked_sub(1)
    } else {
        (at + 1 < now.len()).then_some(at + 1)
    };
    let to = to.ok_or(if up {
        "It is the first phase already."
    } else {
        "It is the last phase already."
    })?;
    let mut moved = now.to_vec();
    moved.swap(at, to);
    let place = |l: &[PhaseDef], p: Option<u32>| p.and_then(|p| position(l, p));
    // what is built in the first phase by default stays in the phase that
    // was first, so nothing changes phase by the move
    let was_first = now[0].id;
    let wrong = model
        .iter()
        .filter(|e| phased(e))
        .filter(|e| {
            let built = place(&moved, e.phase.built.or(Some(was_first))).unwrap_or(0);
            place(&moved, e.phase.gone).is_some_and(|g| g < built)
        })
        .count();
    if wrong > 0 {
        return Err(format!(
            "{} and {} can't swap places: {wrong} element{} would be taken down before {} built.",
            now[at].name,
            now[to].name,
            if wrong == 1 { "" } else { "s" },
            if wrong == 1 { "it is" } else { "they are" },
        ));
    }
    let mut tx =
        edit_list(model, "Reorder phases", |l| *l = moved.clone()).ok_or("Nothing to move.")?;
    // a new first phase: what was built in the old first by default now
    // says so, and what is built in the new first needs not
    if moved[0].id != was_first {
        for e in model.iter().filter(|e| phased(e)) {
            let mut after = e.clone();
            after.phase.built = match e.phase.built {
                None => Some(was_first),
                Some(b) if b == moved[0].id => None,
                b => b,
            };
            if after != *e {
                tx = tx.replace(e.clone(), after);
            }
        }
    }
    Ok(tx)
}

/// Remove phase `id`. Refused, with why, while anything is built or taken
/// down in it, or when it is the only one. Views standing at it move to
/// the phase before it (the one after, for the first).
pub fn remove(model: &Model, id: u32) -> Result<Transaction, String> {
    let now = list(model);
    let at = position(now, id).ok_or("No such phase.")?;
    if now.len() < 2 {
        return Err("A project with phases keeps at least one.".into());
    }
    let (built, down) = uses(model, id);
    if built + down > 0 {
        return Err(format!(
            "{} is not empty: {built} built and {down} taken down in it. Move them to another phase first (Element tab, Time).",
            now[at].name
        ));
    }
    let next = now[if at > 0 { at - 1 } else { 1 }].id;
    let mut tx = edit_list(model, "Remove phase", |l| l.retain(|p| p.id != id))
        .ok_or("Nothing to remove.")?;
    for e in model.iter() {
        if let ElementKind::View(v) = &e.kind {
            if v.phase == Some(id) {
                let mut after = e.clone();
                if let ElementKind::View(v) = &mut after.kind {
                    v.phase = Some(next);
                }
                tx = tx.replace(e.clone(), after);
            }
        }
    }
    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    fn phased_house() -> Document {
        let mut doc = Document::from_model(crate::starter::starter_house());
        let tx = start(doc.model(), true).unwrap();
        doc.commit(tx).unwrap();
        doc
    }

    fn a_wall(m: &Model) -> ElementId {
        m.iter()
            .find(|e| matches!(e.kind, ElementKind::Wall(_)) && !m.dependents(e.id).is_empty())
            .unwrap()
            .id
    }

    #[test]
    fn status_follows_the_phase_stood_at() {
        let l = standard();
        let old = Phasing::default();
        let down = Phasing {
            gone: Some(2),
            ..Default::default()
        };
        let new = Phasing {
            built: Some(3),
            ..Default::default()
        };
        assert_eq!(
            status(&l, old, 0),
            Status::New,
            "the first phase draws plainly"
        );
        assert_eq!(status(&l, old, 2), Status::Existing);
        assert_eq!(status(&l, down, 0), Status::New);
        assert_eq!(status(&l, down, 1), Status::TakenDown);
        assert_eq!(status(&l, down, 2), Status::Absent);
        assert_eq!(status(&l, new, 0), Status::Absent);
        assert_eq!(status(&l, new, 1), Status::Absent);
        assert_eq!(status(&l, new, 2), Status::New);
        // gone in a phase that is not a demolition: just not there
        assert_eq!(
            status(
                &l,
                Phasing {
                    gone: Some(3),
                    ..Default::default()
                },
                2
            ),
            Status::Absent
        );
        // no phases: everything is always there
        assert_eq!(status(&[], new, 0), Status::New);
    }

    #[test]
    fn starting_phases_changes_nothing_on_screen() {
        let doc = phased_house();
        let m = doc.model();
        assert_eq!(list(m).len(), 3);
        for e in m.iter() {
            if let ElementKind::View(v) = &e.kind {
                assert_eq!(v.phase, Some(1), "{}", e.name);
                assert!(
                    model_at(m, v.phase, &[], &[]).is_none(),
                    "nothing is absent from Existing"
                );
            }
        }
        // drawn as the proposed building: all new, views at Proposed
        let mut doc = Document::from_model(crate::starter::starter_house());
        let tx = start(doc.model(), false).unwrap();
        doc.commit(tx).unwrap();
        let m = doc.model();
        let wall = a_wall(m);
        assert_eq!(m.get(wall).unwrap().phase.built, Some(3));
        assert!(model_at(m, Some(3), &[], &[]).is_none());
        assert!(
            model_at(m, Some(1), &[], &[]).unwrap().get(wall).is_none(),
            "not built yet in Existing"
        );
        assert!(start(m, true).is_none(), "phases start once");
    }

    #[test]
    fn deleting_from_a_later_phase_takes_down() {
        let mut doc = phased_house();
        let wall = a_wall(doc.model());
        let doors = doc.model().dependents(wall);
        let n = doc.model().len();
        let (tx, taken) = remove_from(doc.model(), &[wall], Some(3));
        doc.commit(tx).unwrap();
        let m = doc.model();
        assert_eq!(m.len(), n, "nothing is deleted");
        assert!(taken >= 2, "the wall and what is in it");
        assert_eq!(
            m.get(wall).unwrap().phase.gone,
            Some(2),
            "it comes down in Demolition"
        );
        // still there in Existing and in Demolition, gone from Proposed with its doors
        assert!(model_at(m, Some(1), &[], &[]).is_none());
        assert!(model_at(m, Some(2), &[], &[]).is_none());
        assert_eq!(statuses(m, Some(2), &[], &[])[&wall], Status::TakenDown);
        let after = model_at(m, Some(3), &[], &[]).unwrap();
        assert!(after.get(wall).is_none());
        for d in doors {
            assert!(after.get(d).is_none());
        }
        // the same copy for every view at that phase
        assert!(Arc::ptr_eq(
            &after,
            &model_at(m, Some(3), &[], &[]).unwrap()
        ));
        // deleting from the phase it was built in deletes it
        let (tx, taken) = remove_from(m, &[wall], Some(1));
        assert_eq!(taken, 0);
        doc.commit(tx).unwrap();
        assert!(doc.model().get(wall).is_none());
    }

    #[test]
    fn what_is_drawn_belongs_to_the_working_phase() {
        let mut doc = phased_house();
        let wall = a_wall(doc.model());
        let mut copy = doc.model().get(wall).unwrap().clone();
        copy.id = ElementId::new();
        doc.set_working_phase(Some(3));
        doc.commit(Transaction::new("Copy").insert(copy.clone()))
            .unwrap();
        assert_eq!(doc.model().get(copy.id).unwrap().phase.built, Some(3));
        assert_eq!(
            statuses(doc.model(), Some(3), &[], &[])[&copy.id],
            Status::New
        );
        assert_eq!(
            statuses(doc.model(), Some(3), &[], &[])[&wall],
            Status::Existing
        );
        // undone and redone as it was stamped
        assert!(doc.undo());
        assert!(doc.redo());
        assert_eq!(doc.model().get(copy.id).unwrap().phase.built, Some(3));
        // in the first phase it is simply there from the start
        let mut other = copy.clone();
        other.id = ElementId::new();
        doc.set_working_phase(Some(1));
        doc.commit(Transaction::new("Copy").insert(other.clone()))
            .unwrap();
        assert_eq!(doc.model().get(other.id).unwrap().phase, Phasing::default());
    }

    #[test]
    fn a_gone_before_built_is_refused() {
        let mut doc = phased_house();
        let wall = a_wall(doc.model());
        let tx = set(doc.model(), &[wall], "Built in", |p| p.built = Some(3));
        doc.commit(tx).unwrap();
        let tx = set(doc.model(), &[wall], "Taken down in", |p| p.gone = Some(2));
        assert!(tx.ops.is_empty(), "taken down before it is built");
        // phasing is written only when set
        let json = serde_json::to_string(doc.model().get(wall).unwrap()).unwrap();
        assert!(json.contains(r#""phase":{"built":3}"#), "{json}");
        let plain = doc
            .model()
            .iter()
            .find(|e| phased(e) && e.id != wall)
            .unwrap();
        assert!(!serde_json::to_string(plain).unwrap().contains("\"phase\""));
    }

    #[test]
    fn phases_are_added_renamed_moved_and_removed() {
        let mut doc = phased_house();
        // a new one after Proposed, named unlike the others
        let (tx, loft) = add(doc.model(), "Proposed", Some(3)).unwrap();
        doc.commit(tx).unwrap();
        let names = |d: &Document| {
            list(d.model())
                .iter()
                .map(|p| p.name.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names(&doc),
            ["Existing", "Demolition", "Proposed", "Proposed 2"]
        );
        doc.commit(rename(doc.model(), loft, " Loft ").unwrap())
            .unwrap();
        assert!(
            rename(doc.model(), loft, "existing").is_none(),
            "another phase's name"
        );
        assert!(rename(doc.model(), loft, "  ").is_none());
        // moved earlier, the ids stay
        doc.commit(shift(doc.model(), loft, true).unwrap()).unwrap();
        assert_eq!(names(&doc), ["Existing", "Demolition", "Loft", "Proposed"]);
        assert_eq!(position(list(doc.model()), loft), Some(2));
        assert!(shift(doc.model(), 3, false).is_err(), "the last stays last");
        // a wall taken down in Demolition can't have Demolition before Existing
        let wall = a_wall(doc.model());
        let (tx, _) = remove_from(doc.model(), &[wall], Some(3));
        doc.commit(tx).unwrap();
        let why = shift(doc.model(), 2, true).unwrap_err();
        assert!(why.contains("taken down before"), "{why}");
        // a phase with something in it stays; an empty one goes, its views with the one before
        assert!(remove(doc.model(), 2).unwrap_err().contains("not empty"));
        let view = doc
            .model()
            .iter()
            .find(|e| matches!(e.kind, ElementKind::View(_)))
            .unwrap()
            .id;
        doc.edit(view, "phase", |e| {
            if let ElementKind::View(v) = &mut e.kind {
                v.phase = Some(loft);
            }
        })
        .unwrap();
        doc.commit(remove(doc.model(), loft).unwrap()).unwrap();
        assert_eq!(names(&doc), ["Existing", "Demolition", "Proposed"]);
        match &doc.model().get(view).unwrap().kind {
            ElementKind::View(v) => assert_eq!(v.phase, Some(2)),
            _ => unreachable!(),
        }
    }

    #[test]
    fn a_new_first_phase_changes_no_elements_phase() {
        let mut doc = phased_house();
        let wall = a_wall(doc.model());
        let (tx, before) = add(doc.model(), "Before", Some(1)).unwrap();
        doc.commit(tx).unwrap();
        doc.commit(shift(doc.model(), before, true).unwrap())
            .unwrap();
        let l = list(doc.model());
        assert_eq!(l[0].id, before);
        // the wall is still built in Existing, now the second phase
        assert_eq!(doc.model().get(wall).unwrap().phase.built, Some(1));
        assert_eq!(
            status(l, doc.model().get(wall).unwrap().phase, 0),
            Status::Absent
        );
        assert_eq!(uses(doc.model(), before), (0, 0));
        // and back: built in the first by default again
        doc.commit(shift(doc.model(), before, false).unwrap())
            .unwrap();
        assert_eq!(doc.model().get(wall).unwrap().phase.built, None);
    }

    #[test]
    fn an_option_holds_only_what_differs() {
        let mut doc = phased_house();
        let ([a, b], wall) = {
            let (tx, ids) = add_question(doc.model(), "Extension").unwrap();
            doc.commit(tx).unwrap();
            (ids, a_wall(doc.model()))
        };
        assert_eq!(
            chosen(doc.model(), &[]),
            [a],
            "the pick, unless a view says otherwise"
        );
        assert_eq!(chosen(doc.model(), &[b]), [b]);
        // drawn while working in B at Proposed: B's alone
        let mut new = doc.model().get(wall).unwrap().clone();
        new.id = ElementId::new();
        doc.set_working_phase(Some(3));
        doc.set_working_option(Some(b));
        doc.commit(Transaction::new("Draw").insert(new.clone()))
            .unwrap();
        assert_eq!(doc.model().get(new.id).unwrap().phase.option, Some(b));
        // B takes the old wall down; A keeps it
        let (tx, taken) = remove_from_option(doc.model(), &[wall], Some(3), Some(b));
        doc.commit(tx).unwrap();
        assert!(taken >= 1);
        let m = doc.model();
        assert_eq!(m.get(wall).unwrap().phase.gone_option, Some(b));
        let in_a = statuses(m, Some(3), &[a], &[]);
        let in_b = statuses(m, Some(3), &[b], &[]);
        assert_eq!(in_a[&wall], Status::Existing);
        assert_eq!(in_a[&new.id], Status::Absent);
        assert_eq!(in_b[&wall], Status::Absent);
        assert_eq!(in_b[&new.id], Status::New);
        assert_eq!(statuses(m, Some(2), &[b], &[])[&wall], Status::TakenDown);
        assert_eq!(statuses(m, Some(2), &[a], &[])[&wall], Status::Existing);
        assert!(model_at(m, Some(3), &[a], &[])
            .unwrap()
            .get(new.id)
            .is_none());
        assert!(model_at(m, Some(3), &[b], &[]).unwrap().get(wall).is_none());
        assert_eq!(option_uses(m, b).0, 1);
        // B wins: its work becomes the project's own, the question closes
        let (tx, shelved) = settle(m, b).unwrap();
        assert_eq!(shelved, 0);
        doc.commit(tx).unwrap();
        let m = doc.model();
        assert!(questions(m).is_empty());
        assert_eq!(m.get(new.id).unwrap().phase.option, None);
        assert_eq!(m.get(wall).unwrap().phase.gone, Some(2));
        assert_eq!(m.get(wall).unwrap().phase.gone_option, None);
    }

    #[test]
    fn settling_keeps_what_the_other_options_took_down() {
        let mut doc = phased_house();
        let (tx, [a, b]) = add_question(doc.model(), "Extension").unwrap();
        doc.commit(tx).unwrap();
        let wall = a_wall(doc.model());
        let (tx, _) = remove_from_option(doc.model(), &[wall], Some(3), Some(b));
        doc.commit(tx).unwrap();
        let (tx, _) = settle(doc.model(), a).unwrap();
        doc.commit(tx).unwrap();
        assert_eq!(
            doc.model().get(wall).unwrap().phase,
            Phasing::default(),
            "A never took it down"
        );
    }

    #[test]
    fn the_other_option_ghosts_and_shared_parts_hold() {
        let mut doc = phased_house();
        let (tx, [a, b]) = add_question(doc.model(), "Extension").unwrap();
        doc.commit(tx).unwrap();
        let wall = a_wall(doc.model());
        let mut new = doc.model().get(wall).unwrap().clone();
        new.id = ElementId::new();
        doc.set_working_phase(Some(3));
        doc.set_working_option(Some(b));
        doc.commit(Transaction::new("Draw").insert(new.clone()))
            .unwrap();
        let m = doc.model();
        // A shown, B ghosted: B's wall is in the model and reads as a ghost
        assert_eq!(statuses(m, Some(3), &[a], &[])[&new.id], Status::Absent);
        assert_eq!(statuses(m, Some(3), &[a], &[b])[&new.id], Status::Ghost);
        assert!(
            model_at(m, Some(3), &[a], &[b]).is_none(),
            "nothing left out"
        );
        // not at a phase it is not built in yet
        assert_eq!(statuses(m, Some(1), &[a], &[b])[&new.id], Status::Absent);
        // held: a shared wall can't be changed from within B, B's own can
        doc.set_hold_shared(true);
        let moved = |doc: &mut Document, id| {
            doc.edit(id, "Move", |e| {
                if let ElementKind::Wall(w) = &mut e.kind {
                    w.height += 0.1;
                }
            })
        };
        assert!(moved(&mut doc, wall).is_err());
        assert!(moved(&mut doc, new.id).is_ok());
        // but B may take it down, and it can be brought back
        let (tx, _) = remove_from_option(doc.model(), &[wall], Some(3), Some(b));
        doc.commit(tx).unwrap();
        assert!(taken_down(doc.model()).contains(&wall));
        let doors: Vec<ElementId> = doc.model().dependents(wall);
        doc.commit(bring_back(doc.model(), &[wall])).unwrap();
        assert_eq!(doc.model().get(wall).unwrap().phase, Phasing::default());
        for d in doors {
            assert_eq!(
                doc.model().get(d).unwrap().phase.gone,
                None,
                "what came down with it is back"
            );
        }
        // released, or working in what is shared: free again
        doc.set_hold_shared(false);
        assert!(moved(&mut doc, wall).is_ok());
    }

    #[test]
    fn a_settled_question_opens_again() {
        let mut doc = phased_house();
        let (tx, [a, b]) = add_question(doc.model(), "Extension").unwrap();
        doc.commit(tx).unwrap();
        let wall = a_wall(doc.model());
        let mut new = doc.model().get(wall).unwrap().clone();
        new.id = ElementId::new();
        doc.set_working_phase(Some(3));
        doc.set_working_option(Some(b));
        doc.commit(Transaction::new("Draw").insert(new.clone()))
            .unwrap();
        let (tx, shelved) = settle(doc.model(), a).unwrap();
        doc.commit(tx).unwrap();
        assert_eq!(shelved, 1);
        assert!(questions(doc.model()).is_empty());
        let q = shelf(doc.model())[0].id;
        assert_eq!(
            statuses(doc.model(), Some(3), &[], &[])[&new.id],
            Status::Absent,
            "kept, out of every view"
        );
        doc.commit(reopen(doc.model(), q).unwrap()).unwrap();
        assert!(shelf(doc.model()).is_empty());
        assert_eq!(
            statuses(doc.model(), Some(3), &[b], &[])[&new.id],
            Status::New,
            "B's work is back"
        );
        // a new question never takes a shelved one's ids
        let (tx, _) = settle(doc.model(), a).unwrap();
        doc.commit(tx).unwrap();
        let (_, ids) = add_question(doc.model(), "Roof").unwrap();
        assert!(!ids.contains(&a) && !ids.contains(&b));
    }

    #[test]
    fn an_existing_wall_changed_from_a_later_phase_is_altered() {
        let mut doc = phased_house();
        let wall = a_wall(doc.model());
        let doors: Vec<ElementId> = doc.model().dependents(wall);
        let n = doc.model().len();
        let taller = |doc: &mut Document, by: f64| {
            doc.edit_merged(wall, "Length", |e| {
                if let ElementKind::Wall(w) = &mut e.kind {
                    let along = (w.end - w.start).normalize_or_zero();
                    w.end += along * by;
                }
            })
            .unwrap()
        };
        let height = |m: &Model, id| match &m.get(id).unwrap().kind {
            ElementKind::Wall(w) => w.start.distance(w.end),
            _ => unreachable!(),
        };
        let was = height(doc.model(), wall);
        // correcting the survey: one wall, changed in every phase
        doc.set_working_phase(Some(3));
        taller(&mut doc, 0.1);
        assert_eq!(doc.model().len(), n);
        assert_eq!(doc.model().get(wall).unwrap().phase, Phasing::default());
        // altering it: the old one comes down in Demolition, this one is new
        doc.set_alter_existing(true);
        taller(&mut doc, 0.2);
        taller(&mut doc, 0.2);
        assert_eq!(doc.take_altered(), 1, "altered once, however long the drag");
        let m = doc.model();
        assert_eq!(m.get(wall).unwrap().phase.built, Some(3));
        assert!((height(m, wall) - (was + 0.5)).abs() < 1e-9);
        let old = m
            .iter()
            .find(|e| {
                matches!(e.kind, ElementKind::Wall(_))
                    && e.id != wall
                    && e.phase.gone == Some(2)
                    && e.name == m.get(wall).unwrap().name
            })
            .expect("the wall as it was");
        assert!(
            (height(m, old.id) - (was + 0.1)).abs() < 1e-9,
            "as surveyed"
        );
        // its doors and windows: built with the new wall, copies in the old
        let hung = doors
            .iter()
            .filter(|d| matches!(m.get(**d).unwrap().kind, ElementKind::Opening(_)))
            .count();
        assert!(hung > 0);
        for d in &doors {
            if matches!(m.get(*d).unwrap().kind, ElementKind::Opening(_)) {
                assert_eq!(m.get(*d).unwrap().phase.built, Some(3));
            }
        }
        assert_eq!(m.dependents(old.id).len(), hung);
        // each phase shows one of them
        let at = |p| statuses(m, Some(p), &[], &[]);
        assert_eq!(at(1)[&wall], Status::Absent);
        assert_eq!(at(1)[&old.id], Status::New);
        assert_eq!(at(2)[&old.id], Status::TakenDown);
        assert_eq!(at(3)[&old.id], Status::Absent);
        assert_eq!(at(3)[&wall], Status::New);
        // a second change is a plain edit of the new wall
        taller(&mut doc, 0.1);
        assert_eq!(doc.take_altered(), 0);
    }

    #[test]
    fn a_door_taken_down_from_a_wall_that_stays_is_made_good() {
        let mut doc = phased_house();
        let m = doc.model();
        let wall = a_wall(m);
        let door = m
            .dependents(wall)
            .into_iter()
            .find(|d| matches!(m.get(*d).unwrap().kind, ElementKind::Opening(_)))
            .unwrap();
        assert!(making_good(m).is_empty());
        let (tx, _) = remove_from(m, &[door], Some(3));
        doc.commit(tx).unwrap();
        let good = making_good(doc.model());
        assert_eq!(good.len(), 1);
        assert_eq!((good[0].0, good[0].1), (door, wall));
        assert!(good[0].2 > 0.5, "its area: {}", good[0].2);
        // the whole wall down: nothing to make good
        let (tx, _) = remove_from(doc.model(), &[wall], Some(3));
        doc.commit(tx).unwrap();
        assert!(making_good(doc.model()).is_empty());
    }
}

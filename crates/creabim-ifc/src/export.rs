use crate::params::{
    ceiling_props, curtain_props, footing_props, room_props, type_props, Prop, CEILING_PSET,
    CURTAIN_PSET, FOOTING_PSET, ROOM_PSET, TYPE_PSET,
};
use crate::step::{list, point2, point3, real, string, Ref, StepWriter};
use creabim_core::{Element, ElementId, ElementKind, Model, OpeningKind};
use glam::{DVec2, DVec3};
use std::collections::HashMap;

pub struct ExportOptions {
    pub project_name: String,
    pub site_name: String,
    pub building_name: String,
    /// ISO 8601 timestamp for the file header.
    pub timestamp: String,
    /// How walls, slabs and roofs go out (MU20).
    pub body: IfcBody,
}

/// How walls, slabs and roofs are written (MU20). Everything else is the
/// same in all three.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum IfcBody {
    /// Each one IfcWall / IfcSlab / IfcRoof with its layer set and its
    /// plain body: what every program reads and re-imports as such.
    #[default]
    Modelled,
    /// Their bodies cut where walls run through slabs, as CreaBIM draws
    /// and shows them.
    Joined,
    /// The structure stays the element; each other layer is an
    /// IfcCovering covering it (finishes, insulation, membranes).
    Layers,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            project_name: "CreaBIM Project".into(),
            site_name: "Site".into(),
            building_name: "Building".into(),
            timestamp: "2026-01-01T00:00:00".into(),
            body: IfcBody::Modelled,
        }
    }
}

impl ExportOptions {
    /// The names from the model's project information (A7-5), `fallback`
    /// (the file's name) for a project without one.
    pub fn for_model(model: &creabim_core::Model, fallback: &str) -> Self {
        let p = model
            .project_info()
            .map(|(_, p)| p.clone())
            .unwrap_or_default();
        let or = |v: &str, d: &str| {
            if v.trim().is_empty() {
                d.to_string()
            } else {
                v.trim().to_string()
            }
        };
        Self {
            project_name: or(&p.name, fallback),
            site_name: or(&p.site_name, "Site"),
            building_name: or(&p.building_name, "Building"),
            ..Default::default()
        }
    }
}

/// Marks the hatch in an exported material's description, so CreaBIM
/// gets it back on import.
pub(crate) const HATCH_PREFIX: &str = "CreaBIM hatch: ";

struct Ctx {
    w: StepWriter,
    body: Ref,
    /// Plan symbols of furniture and fixtures.
    footprint: Ref,
    axis: Ref,
    z_axis: Ref,
    x_axis: Ref,
    /// Each element's product, for voiding it later.
    products: HashMap<ElementId, Ref>,
    /// Each element's mark, written as its IFC Tag.
    marks: HashMap<ElementId, String>,
    /// The project's phases; empty for a project without them.
    phases: Vec<creabim_core::phases::PhaseDef>,
}

/// The standard common property set of an IFC class that carries the
/// element's Status (PEnum_ElementStatus).
fn common_set(class: &str) -> Option<&'static str> {
    Some(match class {
        c if c.starts_with("IFCWALL") => "Pset_WallCommon",
        c if c.starts_with("IFCSLAB") => "Pset_SlabCommon",
        c if c.starts_with("IFCDOOR") => "Pset_DoorCommon",
        c if c.starts_with("IFCWINDOW") => "Pset_WindowCommon",
        c if c.starts_with("IFCCOLUMN") => "Pset_ColumnCommon",
        c if c.starts_with("IFCBEAM") => "Pset_BeamCommon",
        c if c.starts_with("IFCROOF") => "Pset_RoofCommon",
        c if c.starts_with("IFCSTAIRFLIGHT") => "Pset_StairFlightCommon",
        c if c.starts_with("IFCSTAIR") => "Pset_StairCommon",
        c if c.starts_with("IFCRAILING") => "Pset_RailingCommon",
        c if c.starts_with("IFCCOVERING") => "Pset_CoveringCommon",
        c if c.starts_with("IFCFOOTING") => "Pset_FootingCommon",
        c if c.starts_with("IFCMEMBER") => "Pset_MemberCommon",
        c if c.starts_with("IFCPLATE") => "Pset_PlateCommon",
        c if c.starts_with("IFCBUILDINGELEMENTPROXY") => "Pset_BuildingElementProxyCommon",
        _ => return None,
    })
}

impl Ctx {
    /// An element's Tag attribute: its mark, or unset.
    fn tag(&self, id: ElementId) -> String {
        self.marks
            .get(&id)
            .map_or_else(|| "$".into(), |m| string(m))
    }

    fn guid(&self) -> String {
        string(&ElementId::new().ifc_guid())
    }

    fn dir(&mut self, d: DVec3) -> Ref {
        if d == DVec3::Z {
            return self.z_axis;
        }
        if d == DVec3::X {
            return self.x_axis;
        }
        self.w.add(
            "IFCDIRECTION",
            format!("({},{},{})", real(d.x), real(d.y), real(d.z)),
        )
    }

    fn placement3d(&mut self, at: DVec3, axis: Option<DVec3>, ref_dir: Option<DVec3>) -> Ref {
        let p = self.w.add("IFCCARTESIANPOINT", point3(at.x, at.y, at.z));
        let a = axis.map(|d| self.dir(d).to_string()).unwrap_or("$".into());
        let r = ref_dir
            .map(|d| self.dir(d).to_string())
            .unwrap_or("$".into());
        self.w.add("IFCAXIS2PLACEMENT3D", format!("{p},{a},{r}"))
    }

    fn local_placement(&mut self, rel: Option<Ref>, at: DVec3, ref_dir: Option<DVec3>) -> Ref {
        let axis = ref_dir.map(|_| DVec3::Z);
        let p = self.placement3d(at, axis, ref_dir);
        let rel = rel.map(|r| r.to_string()).unwrap_or("$".into());
        self.w.add("IFCLOCALPLACEMENT", format!("{rel},{p}"))
    }

    fn shape(&mut self, kind: &str, items: &[Ref]) -> Ref {
        let rep = self.body_rep(kind, items);
        self.w
            .add("IFCPRODUCTDEFINITIONSHAPE", format!("$,$,({rep})"))
    }

    fn body_rep(&mut self, kind: &str, items: &[Ref]) -> Ref {
        self.w.add(
            "IFCSHAPEREPRESENTATION",
            format!("{},'Body',{},{}", self.body, string(kind), list(items)),
        )
    }

    /// A property set of single values.
    fn pset(&mut self, name: &str, props: &[(&'static str, Prop)]) -> Ref {
        let refs: Vec<Ref> = props
            .iter()
            .map(|(k, v)| {
                self.w.add(
                    "IFCPROPERTYSINGLEVALUE",
                    format!("{},$,{},$", string(k), v.step()),
                )
            })
            .collect();
        let g = self.guid();
        self.w.add(
            "IFCPROPERTYSET",
            format!("{g},$,{},$,{}", string(name), list(&refs)),
        )
    }

    /// When product `r` of `class` is built and taken down, for a phased
    /// project: the standard Status in its class's common set (EXISTING,
    /// DEMOLISH or NEW, read from the project's last phase), and the
    /// phases by name in a set of CreaBIM's own, so they come back whole.
    /// [`Self::phasing`] for `e`, when the project has phases: for the
    /// writers that add their records by hand (objects, spaces, curtain
    /// walls, holes), which wrote none, so these came back unphased.
    fn phase_of(&mut self, r: Ref, class: &str, e: &Element) {
        if !self.phases.is_empty() && creabim_core::phases::phased(e) {
            self.phasing(r, class, e.phase);
        }
    }

    fn phasing(&mut self, r: Ref, class: &str, p: creabim_core::phases::Phasing) {
        use creabim_core::phases::position;
        let Some(first) = self.phases.first().map(|p| p.name.clone()) else {
            return;
        };
        let name = |id: Option<u32>| {
            id.and_then(|id| self.phases.iter().find(|x| x.id == id))
                .map(|x| x.name.clone())
        };
        let built = name(p.built).unwrap_or(first);
        let gone = name(p.gone);
        let status = if gone.is_some() {
            "DEMOLISH"
        } else if p.built.and_then(|b| position(&self.phases, b)).unwrap_or(0) == 0
            && self.phases.len() > 1
        {
            "EXISTING"
        } else {
            "NEW"
        };
        self.pset_on(
            &[r],
            "CreaBIM_Phasing",
            &[
                ("BuiltIn", Prop::Label(built)),
                ("TakenDownIn", Prop::Label(gone.unwrap_or_default())),
                // the project's phases in order, one that takes things
                // down starred, so the list comes back as it was
                (
                    "Phases",
                    Prop::Text(
                        self.phases
                            .iter()
                            .map(|p| format!("{}{}", p.name, if p.demolition { "*" } else { "" }))
                            .collect::<Vec<_>>()
                            .join("|"),
                    ),
                ),
            ],
        );
        // a class with no common set (furniture, spaces, curtain walls)
        // says it in Bonsai's EPset_Status, as Highland Haven does
        {
            let set = common_set(class).unwrap_or("EPset_Status");
            let v = self.w.add(
                "IFCPROPERTYENUMERATEDVALUE",
                format!("'Status',$,(IFCLABEL('{status}')),$"),
            );
            let g = self.guid();
            let s = self
                .w
                .add("IFCPROPERTYSET", format!("{g},$,{},$,({v})", string(set)));
            let g = self.guid();
            self.w
                .add("IFCRELDEFINESBYPROPERTIES", format!("{g},$,$,$,({r}),{s}"));
        }
    }

    /// A property set, given to `objects`.
    fn pset_on(&mut self, objects: &[Ref], name: &str, props: &[(&'static str, Prop)]) {
        let set = self.pset(name, props);
        let g = self.guid();
        self.w.add(
            "IFCRELDEFINESBYPROPERTIES",
            format!("{g},$,$,$,{},{set}", list(objects)),
        );
    }

    /// A polygon in plan extruded `depth` up from height `z`.
    fn plan_extrusion(&mut self, pts: &[DVec2], z: f64, depth: f64) -> Ref {
        let mut refs: Vec<Ref> = pts
            .iter()
            .map(|p| self.w.add("IFCCARTESIANPOINT", point2(p.x, p.y)))
            .collect();
        refs.push(refs[0]);
        let poly = self.w.add("IFCPOLYLINE", list(&refs));
        let prof = self
            .w
            .add("IFCARBITRARYCLOSEDPROFILEDEF", format!(".AREA.,$,{poly}"));
        let pos = self.placement3d(DVec3::new(0.0, 0.0, z), None, None);
        self.w.add(
            "IFCEXTRUDEDAREASOLID",
            format!("{prof},{pos},{},{}", self.z_axis, real(depth)),
        )
    }

    /// A polygon drawn on a wall's face (`x` along, `y` up) extruded
    /// `depth` through it, from `y = depth` back to `y = 0`.
    fn face_extrusion(&mut self, pts: &[DVec2], depth: f64) -> Ref {
        // a piece that comes to a point has a corner fewer
        let mut pts = pts.to_vec();
        pts.dedup_by(|a, b| a.distance(*b) < 1e-9);
        if pts.len() > 1 && pts[0].distance(pts[pts.len() - 1]) < 1e-9 {
            pts.pop();
        }
        // local Z along -Y, local X along X: the profile's y is up
        let pos = self.placement3d(DVec3::new(0.0, depth, 0.0), Some(-DVec3::Y), Some(DVec3::X));
        self.profile_extrusion(&pts, pos, depth)
    }

    /// A closed polygon (profile plane) extruded `depth` along the
    /// solid's local Z from `position`.
    fn profile_extrusion(&mut self, pts: &[DVec2], position: Ref, depth: f64) -> Ref {
        let mut refs: Vec<Ref> = pts
            .iter()
            .map(|p| self.w.add("IFCCARTESIANPOINT", point2(p.x, p.y)))
            .collect();
        refs.push(refs[0]);
        let poly = self.w.add("IFCPOLYLINE", list(&refs));
        let prof = self
            .w
            .add("IFCARBITRARYCLOSEDPROFILEDEF", format!(".AREA.,$,{poly}"));
        self.w.add(
            "IFCEXTRUDEDAREASOLID",
            format!("{prof},{position},{},{}", self.z_axis, real(depth)),
        )
    }

    /// An IfcOpeningElement of shape `solid` (in `host_pl`'s frame) voiding `host`.
    fn void(&mut self, host: Ref, host_pl: Ref, solid: Ref, name: &str, guid: Option<String>) {
        let pl = self.local_placement(Some(host_pl), DVec3::ZERO, None);
        let shape = self.shape("SweptSolid", &[solid]);
        let guid = guid.unwrap_or_else(|| self.guid());
        let opening = self.w.add(
            "IFCOPENINGELEMENT",
            format!("{guid},$,{},$,$,{pl},{shape},$,.OPENING.", string(name)),
        );
        let g = self.guid();
        self.w
            .add("IFCRELVOIDSELEMENT", format!("{g},$,$,$,{host},{opening}"));
    }

    /// A rectangle `xdim` × `ydim` centred at `center` (profile plane),
    /// extruded `depth` along the solid's local Z.
    fn rect_extrusion(
        &mut self,
        center: DVec2,
        xdim: f64,
        ydim: f64,
        position: Ref,
        depth: f64,
    ) -> Ref {
        let c = self.w.add("IFCCARTESIANPOINT", point2(center.x, center.y));
        let p2 = self.w.add("IFCAXIS2PLACEMENT2D", format!("{c},$"));
        let prof = self.w.add(
            "IFCRECTANGLEPROFILEDEF",
            format!(".AREA.,$,{p2},{},{}", real(xdim), real(ydim)),
        );
        self.w.add(
            "IFCEXTRUDEDAREASOLID",
            format!("{prof},{position},{},{}", self.z_axis, real(depth)),
        )
    }
}

/// Export the model as an IFC4 file.
pub fn export_ifc4(model: &Model, opts: &ExportOptions) -> String {
    export_ifc4_with(model, opts, &creabim_core::progress::quiet)
}

/// [`export_ifc4`], telling `tick` how far it has got.
pub fn export_ifc4_with(
    model: &Model,
    opts: &ExportOptions,
    tick: creabim_core::progress::Tick,
) -> String {
    // groups as the file will hold them, read before copies are folded
    let mut groups: Vec<(creabim_core::groups::GroupDef, Vec<ElementId>)> =
        creabim_core::groups::groups(model)
            .iter()
            .map(|g| (g.clone(), creabim_core::groups::holds(model, g)))
            .collect();
    // CreaBIM's own design options go out as groups too, as Bonsai keeps
    // them: one a option, holding what is in that option alone
    groups.extend(option_groups(model, &groups));
    let folded = fold_copies(model);
    let model: &Model = folded.as_ref().unwrap_or(model);
    let total = model.len();
    tick(0, total, "Preparing the shapes");
    let mut w = StepWriter::default();
    let z_axis = w.add("IFCDIRECTION", "(0.,0.,1.)");
    let x_axis = w.add("IFCDIRECTION", "(1.,0.,0.)");
    let origin = w.add("IFCCARTESIANPOINT", "(0.,0.,0.)");
    let wcs = w.add("IFCAXIS2PLACEMENT3D", format!("{origin},$,$"));
    let model_ctx = w.add(
        "IFCGEOMETRICREPRESENTATIONCONTEXT",
        format!("$,'Model',3,1.E-05,{wcs},$"),
    );
    let body = w.add(
        "IFCGEOMETRICREPRESENTATIONSUBCONTEXT",
        format!("'Body','Model',*,*,*,*,{model_ctx},$,.MODEL_VIEW.,$"),
    );
    let axis = w.add(
        "IFCGEOMETRICREPRESENTATIONSUBCONTEXT",
        format!("'Axis','Model',*,*,*,*,{model_ctx},$,.GRAPH_VIEW.,$"),
    );
    let footprint = w.add(
        "IFCGEOMETRICREPRESENTATIONSUBCONTEXT",
        format!("'FootPrint','Model',*,*,*,*,{model_ctx},$,.PLAN_VIEW.,$"),
    );
    let mut c = Ctx {
        w,
        body,
        footprint,
        axis,
        z_axis,
        x_axis,
        products: HashMap::new(),
        marks: element_marks(model),
        phases: creabim_core::phases::list(model).to_vec(),
    };

    let units = [
        c.w.add("IFCSIUNIT", "*,.LENGTHUNIT.,$,.METRE."),
        c.w.add("IFCSIUNIT", "*,.AREAUNIT.,$,.SQUARE_METRE."),
        c.w.add("IFCSIUNIT", "*,.VOLUMEUNIT.,$,.CUBIC_METRE."),
        c.w.add("IFCSIUNIT", "*,.PLANEANGLEUNIT.,$,.RADIAN."),
    ];
    let unit_assignment = c.w.add("IFCUNITASSIGNMENT", list(units));
    // the file's project, site and building keep their GlobalIds, so what
    // it tied to them (Bonsai's sheets) finds them again
    let anchor = |c: &Ctx, key: &str| {
        model
            .ifc_data()
            .anchors
            .get(key)
            .map_or_else(|| c.guid(), |g| string(g))
    };
    let project = c.w.add(
        "IFCPROJECT",
        format!(
            "{},$,{},$,$,$,$,({model_ctx}),{unit_assignment}",
            anchor(&c, "project"),
            string(&opts.project_name)
        ),
    );

    let site_pl = c.local_placement(None, DVec3::ZERO, None);
    // where the site is in the world, from the project's information:
    // latitude and longitude as degrees, minutes, seconds and millionths
    // of a second, and the height of the project's zero
    let info = model.project_info().map(|p| p.1);
    let angle = |v: Option<f64>| {
        v.filter(|v| v.is_finite()).map_or("$".to_string(), |v| {
            let sign = if v < 0.0 { -1i64 } else { 1 };
            let total = (v.abs() * 3_600_000_000.0).round() as i64;
            let (d, rest) = (total / 3_600_000_000, total % 3_600_000_000);
            let (m, rest) = (rest / 60_000_000, rest % 60_000_000);
            let (sec, millionths) = (rest / 1_000_000, rest % 1_000_000);
            format!(
                "({},{},{},{})",
                sign * d,
                sign * m,
                sign * sec,
                sign * millionths
            )
        })
    };
    let located = info.is_some_and(|i| i.latitude.is_some() && i.longitude.is_some());
    // and on the map: the model's origin in a map coordinate system, its
    // axes along the map's
    if let Some(map) = info.and_then(|i| i.map.as_ref()) {
        let name = match (map.epsg(), map.crs.trim()) {
            (Some(code), _) => format!("EPSG:{code}"),
            (None, "") => "Unknown".to_string(),
            (None, name) => name.to_string(),
        };
        let crs =
            c.w.add("IFCPROJECTEDCRS", format!("{},$,$,$,$,$,$", string(&name)));
        c.w.add(
            "IFCMAPCONVERSION",
            format!(
                "{model_ctx},{crs},{},{},{},1.,0.,$",
                real(map.easting),
                real(map.northing),
                real(info.map_or(0.0, |i| i.elevation))
            ),
        );
    }
    let site = c.w.add(
        "IFCSITE",
        format!(
            "{},$,{},$,$,{site_pl},$,$,.ELEMENT.,{},{},{},$,$",
            anchor(&c, "site"),
            string(&opts.site_name),
            angle(info.and_then(|i| i.latitude)),
            angle(info.and_then(|i| i.longitude)),
            match info {
                Some(i) if located => real(i.elevation),
                _ => "$".to_string(),
            }
        ),
    );
    let bldg_pl = c.local_placement(Some(site_pl), DVec3::ZERO, None);
    let building = c.w.add(
        "IFCBUILDING",
        format!(
            "{},$,{},$,$,{bldg_pl},$,$,.ELEMENT.,$,$,$",
            anchor(&c, "building"),
            string(&opts.building_name)
        ),
    );

    // Storeys, one per level.
    let mut storeys: HashMap<ElementId, (Ref, Ref)> = HashMap::new();
    let mut storey_order = Vec::new();
    for (e, l) in model.levels() {
        let pl = c.local_placement(Some(bldg_pl), DVec3::new(0.0, 0.0, l.elevation), None);
        let s = c.w.add(
            "IFCBUILDINGSTOREY",
            format!(
                "{},$,{},$,$,{pl},$,$,.ELEMENT.,{}",
                string(&e.id.ifc_guid()),
                string(&e.name),
                real(l.elevation)
            ),
        );
        storeys.insert(e.id, (s, pl));
        storey_order.push(s);
    }

    let mut contained: HashMap<Option<ElementId>, Vec<Ref>> = HashMap::new();
    // what is part of the site (outdoor steps, a retaining wall) is
    // placed in the site, not in a storey
    let mut on_site: Vec<Ref> = Vec::new();
    let mut wall_refs: HashMap<ElementId, (Ref, Ref)> = HashMap::new(); // entity, placement
                                                                        // how far each wall's axis (its location line) is from its centreline
    let mut wall_axes: HashMap<ElementId, f64> = HashMap::new();
    let joins = creabim_core::wall_joins(model);
    let mut by_material: HashMap<ElementId, Vec<Ref>> = HashMap::new();
    // layered elements, associated with their assembly's layer set instead
    let mut layered: Vec<(Ref, ElementId, LayerUse)> = Vec::new();
    // boolean cuts: cutters are construction aids and stay out; what they
    // cut is written as its final shape, tessellated
    let cuts = creabim_geom::scene::cuts(model);
    let cutters: std::collections::HashSet<ElementId> = cuts.iter().map(|c| c.1).collect();
    // screen walls too: their holes are written in their shape
    let screens: Vec<ElementId> = model
        .iter()
        .filter(|e| creabim_core::screen::screen_of(model, e).is_some())
        .map(|e| e.id)
        .collect();
    let mut cut_meshes: HashMap<ElementId, creabim_geom::mesh::TriMesh> =
        if cuts.is_empty() && screens.is_empty() {
            HashMap::new()
        } else {
            let targets: std::collections::HashSet<ElementId> =
                cuts.iter().map(|c| c.0).chain(screens).collect();
            let mut all = creabim_geom::scene::element_meshes(
                &creabim_geom::scene::build_scene_for_exchange(model),
            );
            all.retain(|id, _| targets.contains(id));
            all
        };
    // MU20: joined bodies, or only the structural core of a layered one,
    // written as their final shapes
    let joins_now = creabim_core::wall_joins(model);
    let joinable = |e: &Element| match &e.kind {
        ElementKind::Wall(w) => w.curtain.is_none(),
        ElementKind::Slab(_) | ElementKind::Roof(_) => true,
        _ => false,
    };
    match opts.body {
        IfcBody::Modelled => {}
        IfcBody::Joined => {
            let joined =
                creabim_geom::scene::element_meshes(&creabim_geom::scene::build_scene(model));
            for e in model.iter().filter(|e| joinable(e)) {
                if let Some(m) = joined.get(&e.id) {
                    cut_meshes.insert(e.id, m.clone());
                }
            }
        }
        IfcBody::Layers => {
            for e in model.iter().filter(|e| joinable(e)) {
                let parts = creabim_geom::scene::layer_parts(model, e, &joins_now);
                if parts.len() < 2 {
                    continue;
                }
                let mut core = creabim_geom::mesh::TriMesh::default();
                for (l, m) in &parts {
                    if l.core {
                        core.append(m);
                    }
                }
                if !core.indices.is_empty() {
                    cut_meshes.insert(e.id, core);
                }
            }
        }
    }
    let layers_mode = opts.body == IfcBody::Layers;
    let elevation =
        |l: Option<ElementId>| l.and_then(|l| model.level(l)).map_or(0.0, |l| l.elevation);
    // the products of each element type
    let mut typed: HashMap<ElementId, Vec<Ref>> = HashMap::new();
    // ceilings over rooms, to tie to their spaces
    let mut ceilings: Vec<(ElementId, Ref)> = vec![];
    let mut of_type = |e: &Element, r: Ref| {
        if let Some(t) = &e.of_type {
            typed.entry(t.id).or_default().push(r);
        }
    };

    for (i, e) in model.iter().enumerate() {
        if i % 8 == 0 {
            tick(i, total, creabim_core::progress::step_of(e.ifc_class()));
        }
        if cutters.contains(&e.id) {
            continue;
        }
        let cut = cut_meshes.get(&e.id);
        match &e.kind {
            ElementKind::Wall(
                wall @ creabim_core::Wall {
                    curtain: Some(grid),
                    ..
                },
            ) => {
                // a curtain wall: no body of its own, aggregating its
                // mullions (IfcMember) and panels (IfcPlate)
                let Some(&(_, spl)) = storeys.get(&wall.level) else {
                    continue;
                };
                let d = wall.direction();
                let pl = c.local_placement(
                    Some(spl),
                    DVec3::new(wall.start.x, wall.start.y, wall.base_offset),
                    Some(DVec3::new(d.x, d.y, 0.0)),
                );
                let r = c.w.add(
                    "IFCCURTAINWALL",
                    format!(
                        "{},$,{},$,$,{pl},$,$,.NOTDEFINED.",
                        string(&e.id.ifc_guid()),
                        string(&e.name)
                    ),
                );
                c.phase_of(r, "IFCCURTAINWALL", e);
                wall_refs.insert(e.id, (r, pl));
                wall_axes.insert(e.id, 0.0);
                if e.is_site_part() {
                    on_site.push(r);
                } else {
                    contained.entry(Some(wall.level)).or_default().push(r);
                }
                of_type(e, r);
                let openings: Vec<&creabim_core::Opening> = model
                    .dependents(e.id)
                    .into_iter()
                    .filter_map(|o| match &model.get(o)?.kind {
                        ElementKind::Opening(o) => Some(o),
                        _ => None,
                    })
                    .collect();
                let mut parts = vec![];
                let mut solid_parts = vec![];
                // under the roof it is attached to, sloped where it is
                let at = creabim_geom::curtain::setting(model, wall);
                for b in creabim_geom::curtain::curtain_boxes_under(wall, grid, &openings, &at) {
                    use creabim_geom::curtain::CurtainPart;
                    let size = b.hi - b.lo;
                    let ppl = c.local_placement(Some(pl), b.lo, None);
                    let solid = match b.zs {
                        Some([b0, b1, t0, t1]) => {
                            let z = b.lo.z;
                            c.face_extrusion(
                                &[
                                    DVec2::new(0.0, b0 - z),
                                    DVec2::new(size.x, b1 - z),
                                    DVec2::new(size.x, t1 - z),
                                    DVec2::new(0.0, t0 - z),
                                ],
                                size.y,
                            )
                        }
                        None => {
                            let pos = c.placement3d(DVec3::ZERO, None, None);
                            c.rect_extrusion(size.truncate() / 2.0, size.x, size.y, pos, size.z)
                        }
                    };
                    let shape = c.shape("SweptSolid", &[solid]);
                    let (class, tail, what) = match b.part {
                        CurtainPart::Mullion => ("IFCMEMBER", ",$,.MULLION.", "mullion"),
                        CurtainPart::Seal => ("IFCMEMBER", ",$,.MULLION.", "joint"),
                        CurtainPart::Glass => ("IFCPLATE", ",$,.CURTAIN_PANEL.", "glass"),
                        CurtainPart::Panel => ("IFCPLATE", ",$,.CURTAIN_PANEL.", "panel"),
                    };
                    let g = c.guid();
                    let p = c.w.add(
                        class,
                        format!(
                            "{g},$,{},$,$,{ppl},{shape}{tail}",
                            string(&format!("{} {what}", e.name))
                        ),
                    );
                    parts.push(p);
                    if b.part != CurtainPart::Glass {
                        solid_parts.push(p);
                    }
                }
                if !parts.is_empty() {
                    let g = c.guid();
                    c.w.add(
                        "IFCRELAGGREGATES",
                        format!("{g},$,$,$,{r},{}", list(&parts)),
                    );
                }
                if let Some((m, _)) = creabim_core::material_of(model, e) {
                    by_material.entry(m.id).or_default().extend(solid_parts);
                }
                let mut props = curtain_props(grid);
                props.push(("Length", Prop::Length(wall.length())));
                props.push(("Height", Prop::Length(wall.height)));
                c.pset_on(&[r], CURTAIN_PSET, &props);
            }
            ElementKind::Wall(wall) => {
                let Some(&(_, spl)) = storeys.get(&wall.level) else {
                    continue;
                };
                let d = wall.direction();
                let n = d.perp();
                // the wall's axis is its location line: its placement sits
                // on it and the footprint is measured from it
                let loc = creabim_core::walls::location_offset(model, wall);
                let origin = wall.start + n * loc;
                let pl = c.local_placement(
                    Some(spl),
                    DVec3::new(origin.x, origin.y, wall.base_offset),
                    Some(DVec3::new(d.x, d.y, 0.0)),
                );
                wall_axes.insert(e.id, loc);
                // footprint in wall-local coordinates, following its joins,
                // one piece between each pair of walls crossing it
                let ends = joins.get(&e.id).cloned().unwrap_or_default();
                let t = wall.thickness / 2.0;
                let asm = creabim_core::assembly_of(model, e);
                // a layered wall's core may run on past its faces at a T
                let core = asm.map_or((-t, t), |(_, a)| creabim_core::assembly::wall_core(a, wall));
                // an attached top is cut by the undersides over it
                let top = creabim_core::walls::top_profile(model, wall);
                let z0 = elevation(Some(wall.level)) + wall.base_offset;
                let depth = top.as_ref().map_or(wall.height, |tp| {
                    let steps = 16;
                    (0..=steps)
                        .flat_map(|i| {
                            let p = wall.start.lerp(wall.end, i as f64 / steps as f64);
                            [p + n * t, p - n * t]
                        })
                        .filter_map(|p| tp.z_at(p))
                        .map(|z| z - z0)
                        .fold(wall.height, f64::max)
                });
                let mut solids = vec![];
                for local in ends.local_outlines(wall, core) {
                    let mut pts: Vec<Ref> = local
                        .iter()
                        .map(|p| c.w.add("IFCCARTESIANPOINT", point2(p.x, p.y - loc)))
                        .collect();
                    pts.push(pts[0]);
                    let poly = c.w.add("IFCPOLYLINE", list(&pts));
                    let prof =
                        c.w.add("IFCARBITRARYCLOSEDPROFILEDEF", format!(".AREA.,$,{poly}"));
                    let pos = c.placement3d(DVec3::ZERO, None, None);
                    let mut solid = c.w.add(
                        "IFCEXTRUDEDAREASOLID",
                        format!("{prof},{pos},{},{}", c.z_axis, real(depth)),
                    );
                    if let Some(tp) = &top {
                        // wall-local: x along the wall from its axis start
                        let to_local = |p: DVec2| {
                            let q = p - origin;
                            DVec2::new(q.dot(d), q.dot(n))
                        };
                        for f in &tp.faces {
                            solid = clip_under(&mut c, solid, f, to_local, d, n, origin, z0);
                        }
                    }
                    solids.push(solid);
                }
                let clipped = top.is_some();
                // the axis lets other tools (and our importer) recover the
                // location line whatever the joins did to the ends
                let a0 = c.w.add("IFCCARTESIANPOINT", point2(0.0, 0.0));
                let a1 = c.w.add("IFCCARTESIANPOINT", point2(wall.length(), 0.0));
                let axis_line = c.w.add("IFCPOLYLINE", format!("({a0},{a1})"));
                let axis_rep = c.w.add(
                    "IFCSHAPEREPRESENTATION",
                    format!("{},'Axis','Curve2D',({axis_line})", c.axis),
                );
                let body_rep = match cut {
                    Some(m) => {
                        let fs = tessellate(&mut c, m, |p| {
                            let q = p.truncate() - origin;
                            DVec3::new(q.dot(d), q.dot(n), p.z - z0)
                        });
                        c.body_rep("Tessellation", &[fs])
                    }
                    None if clipped => c.body_rep("Clipping", &solids),
                    None => c.body_rep("SweptSolid", &solids),
                };
                let shape = c.w.add(
                    "IFCPRODUCTDEFINITIONSHAPE",
                    format!("$,$,({body_rep},{axis_rep})"),
                );
                let r = product(&mut c, "IFCWALL", e, pl, shape, ",$,.STANDARD.");
                of_type(e, r);
                wall_refs.insert(e.id, (r, pl));
                if e.is_site_part() {
                    on_site.push(r);
                } else {
                    contained.entry(Some(wall.level)).or_default().push(r);
                }
                if cut.is_none() || layers_mode {
                    let z = elevation(Some(wall.level));
                    write_layer_parts(
                        &mut c,
                        model,
                        e,
                        r,
                        spl,
                        z,
                        &joins,
                        &mut by_material,
                        layers_mode,
                    );
                }
                if let Some((a, _)) = asm {
                    // layers run across the wall from its exterior face,
                    // measured from the axis
                    let (sense, offset) = if wall.flip {
                        (false, t - loc)
                    } else {
                        (true, -t - loc)
                    };
                    layered.push((r, a.id, LayerUse::Usage("AXIS2", sense, offset)));
                } else if let Some((m, _)) = creabim_core::material_of(model, e) {
                    by_material.entry(m.id).or_default().push(r);
                }
            }
            ElementKind::Slab(slab) => {
                let Some(&(_, spl)) = storeys.get(&slab.level) else {
                    continue;
                };
                let pl = c.local_placement(Some(spl), DVec3::ZERO, None);
                // arcs go out in short straight pieces
                let mut pts: Vec<Ref> = slab
                    .outline()
                    .iter()
                    .map(|p| c.w.add("IFCCARTESIANPOINT", point2(p.x, p.y)))
                    .collect();
                pts.push(pts[0]);
                let poly = c.w.add("IFCPOLYLINE", list(&pts));
                let prof =
                    c.w.add("IFCARBITRARYCLOSEDPROFILEDEF", format!(".AREA.,$,{poly}"));
                let pos = c.placement3d(
                    DVec3::new(0.0, 0.0, slab.top_offset - slab.thickness),
                    None,
                    None,
                );
                let solid = c.w.add(
                    "IFCEXTRUDEDAREASOLID",
                    format!("{prof},{pos},{},{}", c.z_axis, real(slab.thickness)),
                );
                let shape = match cut {
                    Some(m) => {
                        let z0 = elevation(Some(slab.level));
                        let fs = tessellate(&mut c, m, |p| p - DVec3::Z * z0);
                        c.shape("Tessellation", &[fs])
                    }
                    None => c.shape("SweptSolid", &[solid]),
                };
                let r = product(&mut c, "IFCSLAB", e, pl, shape, ",$,.FLOOR.");
                of_type(e, r);
                if e.is_site_part() {
                    on_site.push(r);
                } else {
                    contained.entry(Some(slab.level)).or_default().push(r);
                }
                if cut.is_none() || layers_mode {
                    let z = elevation(Some(slab.level));
                    write_layer_parts(
                        &mut c,
                        model,
                        e,
                        r,
                        spl,
                        z,
                        &joins,
                        &mut by_material,
                        layers_mode,
                    );
                }
                // holes, and shafts passing through, as openings voiding it
                for (i, h) in creabim_core::slab::holes_of(model, e).iter().enumerate() {
                    let z = slab.top_offset - slab.thickness - 0.05;
                    let solid = c.plan_extrusion(h, z, slab.thickness + 0.1);
                    c.void(r, pl, solid, &format!("{} hole {}", e.name, i + 1), None);
                }
                if let Some((a, _)) = creabim_core::assembly_of(model, e) {
                    // layers run down from the top
                    layered.push((r, a.id, LayerUse::Usage("AXIS3", false, slab.top_offset)));
                } else if let Some((m, _)) = creabim_core::material_of(model, e) {
                    by_material.entry(m.id).or_default().push(r);
                }
            }
            ElementKind::Ceiling(ceiling) => {
                let (Some(&(_, spl)), Some(outline)) =
                    (storeys.get(&ceiling.level), ceiling.outline(model))
                else {
                    continue;
                };
                let pl = c.local_placement(Some(spl), DVec3::ZERO, None);
                let solid = c.plan_extrusion(&outline, ceiling.height, ceiling.thickness);
                let shape = c.shape("SweptSolid", &[solid]);
                let r = product(&mut c, "IFCCOVERING", e, pl, shape, ",$,.CEILING.");
                contained.entry(Some(ceiling.level)).or_default().push(r);
                let room = ceiling.room.filter(|r| model.get(*r).is_some());
                let props = ceiling_props(ceiling, room.map(|r| r.ifc_guid()));
                c.pset_on(&[r], CEILING_PSET, &props);
                if let Some((m, _)) = creabim_core::material_of(model, e) {
                    by_material.entry(m.id).or_default().push(r);
                }
                if let Some(room) = room {
                    ceilings.push((room, r));
                }
            }
            ElementKind::FreeMesh(f) => {
                let spl = f
                    .level
                    .and_then(|l| storeys.get(&l))
                    .map(|s| s.1)
                    .unwrap_or(bldg_pl);
                let pl = c.local_placement(Some(spl), f.origin, None);
                // the shape as shown, with its modifiers applied
                let mesh = f.evaluated().mesh;
                let coords: Vec<String> = mesh
                    .vertices
                    .iter()
                    .map(|v| point3(v.x, v.y, v.z))
                    .collect();
                let pts =
                    c.w.add("IFCCARTESIANPOINTLIST3D", format!("({})", coords.join(",")));
                let mut tris = Vec::new();
                for face in 0..mesh.faces.len() {
                    for [a, b, d] in mesh.face_triangles(face) {
                        tris.push(format!("({},{},{})", a + 1, b + 1, d + 1));
                    }
                }
                let fs = match cut {
                    Some(m) => {
                        let at = f.origin + DVec3::Z * elevation(f.level);
                        tessellate(&mut c, m, |p| p - at)
                    }
                    None => c.w.add(
                        "IFCTRIANGULATEDFACESET",
                        format!("{pts},$,.T.,({}),$", tris.join(",")),
                    ),
                };
                let shape = c.shape("Tessellation", &[fs]);
                let (class, tail) = free_mesh_entity(&f.ifc_class);
                let r = product(&mut c, &class, e, pl, shape, &tail);
                if crate::import::could_be_native(&class) {
                    // so CreaBIM reads it back as the mesh it is
                    let props = [("Kind", Prop::Label("FreeMesh".into()))];
                    c.pset_on(&[r], crate::import::MEMBER_PSET, &props);
                }
                contained.entry(f.level).or_default().push(r);
                if let Some((m, _)) = creabim_core::material_of(model, e) {
                    by_material.entry(m.id).or_default().push(r);
                }
            }
            // a void is an opening in what it cuts, written below
            ElementKind::Part(p) if p.void => {}
            k if creabim_core::members::is_member(k) => {
                let level = creabim_core::members::level_of(k);
                let Some(&(_, spl)) = level.and_then(|l| storeys.get(&l)) else {
                    continue;
                };
                // the shape as built, tessellated in the storey's frame
                let mesh = match cut {
                    Some(m) => Some(m.clone()),
                    None => creabim_geom::members::member_mesh(model, e).map(|m| m.0),
                };
                let Some(mesh) = mesh else { continue };
                let pl = c.local_placement(Some(spl), DVec3::ZERO, None);
                let z0 = elevation(level);
                let fs = tessellate(&mut c, &mesh, |p| p - DVec3::Z * z0);
                let shape = c.shape("Tessellation", &[fs]);
                let (class, tail) = match k {
                    ElementKind::Part(_) | ElementKind::Site(_) => part_entity(e),
                    _ => {
                        let (class, tail) = member_entity(model, k);
                        (class.to_string(), tail)
                    }
                };
                let r = product(&mut c, &class, e, pl, shape, &tail);
                of_type(e, r);
                if e.is_site_part() {
                    on_site.push(r);
                } else {
                    contained.entry(level).or_default().push(r);
                }
                if let ElementKind::Site(site) = k {
                    // a terrain's earthworks, for take-off in other tools
                    if let Some(sh) =
                        creabim_geom::earth::shaped(model, site).filter(|s| s.changed())
                    {
                        let q = [
                            c.w.add(
                                "IFCQUANTITYVOLUME",
                                format!("'CutVolume',$,$,{},$", real(sh.cut)),
                            ),
                            c.w.add(
                                "IFCQUANTITYVOLUME",
                                format!("'FillVolume',$,$,{},$", real(sh.fill)),
                            ),
                            c.w.add(
                                "IFCQUANTITYAREA",
                                format!("'WorkedArea',$,$,{},$", real(sh.area)),
                            ),
                        ];
                        let g = c.guid();
                        let quantities = c.w.add(
                            "IFCELEMENTQUANTITY",
                            format!("{g},$,'CreaBIM_Earthworks',$,$,{}", list(q)),
                        );
                        let g = c.guid();
                        c.w.add(
                            "IFCRELDEFINESBYPROPERTIES",
                            format!("{g},$,$,$,({r}),{quantities}"),
                        );
                    }
                }
                if let ElementKind::Footing(f) = k {
                    // its own values, so CreaBIM reads it back as a footing
                    let host = f.host.filter(|h| model.get(*h).is_some());
                    let props = footing_props(f, host.map(|h| h.ifc_guid()));
                    c.pset_on(&[r], FOOTING_PSET, &props);
                } else if let Ok(json) = serde_json::to_string(k) {
                    // its own definition, so CreaBIM reads it back as the
                    // same column, beam, roof, stair or railing
                    let props = [("Definition", Prop::Text(json))];
                    c.pset_on(&[r], crate::import::MEMBER_PSET, &props);
                }
                if cut.is_none() || layers_mode {
                    write_layer_parts(
                        &mut c,
                        model,
                        e,
                        r,
                        spl,
                        z0,
                        &joins,
                        &mut by_material,
                        layers_mode,
                    );
                }
                if let ElementKind::Roof(roof) = k {
                    // the mesh has its holes already; say so for other tools
                    let vt = roof.vertical_thickness();
                    let z = roof.base_offset - vt - 0.1;
                    for (i, h) in creabim_core::slab::holes_of(model, e).iter().enumerate() {
                        let solid = c.plan_extrusion(h, z, roof.peak() + vt + 0.2);
                        c.void(r, pl, solid, &format!("{} hole {}", e.name, i + 1), None);
                    }
                }
                if let Some((a, _)) = creabim_core::assembly_of(model, e) {
                    // a roof's shape isn't a straight extrusion: the set itself
                    layered.push((r, a.id, LayerUse::Set));
                } else if let Some((m, _)) = creabim_core::material_of(model, e) {
                    by_material.entry(m.id).or_default().push(r);
                }
            }
            _ => {}
        }
    }

    // A void is an opening in each element it cuts, in its own shape.
    for (target, void) in creabim_core::parts::void_cuts(model) {
        let (Some(host), Some(v)) = (c.products.get(&target).copied(), model.get(void)) else {
            continue;
        };
        let ElementKind::Part(p) = &v.kind else {
            continue;
        };
        let (Some(&(_, spl)), Some(mesh)) = (
            storeys.get(&p.level),
            creabim_geom::parts::part_mesh(model, p),
        ) else {
            continue;
        };
        let z0 = elevation(Some(p.level));
        let pl = c.local_placement(Some(spl), DVec3::ZERO, None);
        let fs = tessellate(&mut c, &mesh, |q| q - DVec3::Z * z0);
        let shape = c.shape("Tessellation", &[fs]);
        let name = model.get(target).map_or("", |t| t.name.as_str());
        let opening = c.w.add(
            "IFCOPENINGELEMENT",
            format!(
                "{},$,{},$,$,{pl},{shape},$,.OPENING.",
                c.guid(),
                string(&format!("{} in {name}", v.name))
            ),
        );
        let g = c.guid();
        c.w.add("IFCRELVOIDSELEMENT", format!("{g},$,$,$,{host},{opening}"));
    }

    // Openings need their host wall exported first, so they get a second pass.
    for e in model.iter() {
        let ElementKind::Opening(o) = &e.kind else {
            continue;
        };
        // a roof window: an opening square to its roof's slope, voiding
        // the roof, filled by a skylight placed in the slope
        if let (Some(seat), Some(roof)) = (
            creabim_core::roofwindow::seat(model, o),
            creabim_core::roofwindow::roof_of(model, o),
        ) {
            let (Some(&roof_r), Some(&(_, spl))) =
                (c.products.get(&o.host), storeys.get(&roof.level))
            else {
                continue;
            };
            let Some((local, wall, _)) = creabim_geom::opening::roof_window(model, o) else {
                continue;
            };
            let z0 = elevation(Some(roof.level));
            // its own axes: x across the slope, y into the roof, z up it
            let place = |c: &mut Ctx, at: DVec3| {
                let p = c.placement3d(at, Some(seat.up), Some(seat.across));
                c.w.add("IFCLOCALPLACEMENT", format!("{spl},{p}"))
            };
            let pl = place(&mut c, seat.origin - DVec3::Z * z0);
            let reach = seat.thickness + 1.0;
            let pos = c.placement3d(DVec3::ZERO, None, None);
            let solid = c.rect_extrusion(
                DVec2::new(o.width / 2.0, 0.0),
                o.width,
                2.0 * reach,
                pos,
                o.height,
            );
            let shape = c.shape("SweptSolid", &[solid]);
            let opening = c.w.add(
                "IFCOPENINGELEMENT",
                format!(
                    "{},$,{},$,$,{pl},{shape},$,.OPENING.",
                    c.guid(),
                    string(&format!("{} opening", e.name))
                ),
            );
            let g = c.guid();
            c.w.add(
                "IFCRELVOIDSELEMENT",
                format!("{g},$,$,$,{roof_r},{opening}"),
            );
            // the window, its frame standing proud of the roof as built
            let depth =
                creabim_geom::opening::Fitted::of(&local, &wall).map_or(0.1, |f| f.frame.depth);
            let lift = creabim_geom::opening::roof_lift(&local, &wall, depth);
            let fill_pl = c.local_placement(Some(pl), DVec3::new(0.0, -lift, 0.0), None);
            let mut parts = Vec::new();
            for part in creabim_geom::opening::opening_parts(&local, &wall)
                .into_iter()
                // its upstand, collars and lining (the splayed boards are
                // shaped, not boxes, and are left out of the IFC for now)
                .chain(creabim_geom::opening::roof_build_parts(
                    &local,
                    &wall,
                    seat.pitch(),
                ))
            {
                if let creabim_geom::opening::Shaped::Box(b) = part {
                    let size = b.hi - b.lo;
                    let pos = c.placement3d(b.lo, None, None);
                    parts.push(c.rect_extrusion(
                        size.truncate() / 2.0,
                        size.x,
                        size.y,
                        pos,
                        size.z,
                    ));
                }
            }
            if parts.is_empty() {
                continue;
            }
            let fill_shape = c.shape("SweptSolid", &parts);
            let tail = format!(
                ",$,{},{},.SKYLIGHT.,{}",
                real(o.height),
                real(o.width),
                window_partitioning(&o.frame.grid)
            );
            let fill = product(&mut c, "IFCWINDOW", e, fill_pl, fill_shape, &tail);
            of_type(e, fill);
            let mut own = vec![];
            if let Some(at) = o.roof_at {
                own.push((
                    "RoofAt",
                    crate::params::Prop::Label(format!("{} {}", at.x, at.y)),
                ));
            }
            let grid = serde_json::to_string(&o.frame.grid).unwrap_or_default();
            own.push(("CellGrid", crate::params::Prop::Text(grid)));
            let built = &o.frame.construction;
            if !built.is_none() {
                let text = serde_json::to_string(built).unwrap_or_default();
                own.push(("Construction", crate::params::Prop::Text(text)));
            }
            c.pset_on(&[fill], crate::import::OPENING_PSET, &own);
            let g = c.guid();
            c.w.add("IFCRELFILLSELEMENT", format!("{g},$,$,$,{opening},{fill}"));
            if let Some((m, _)) = creabim_core::material_of(model, e) {
                by_material.entry(m.id).or_default().push(fill);
            }
            contained.entry(Some(roof.level)).or_default().push(fill);
            continue;
        }
        let Some(&(wall_r, wall_pl)) = wall_refs.get(&o.host) else {
            continue;
        };
        let Some(ElementKind::Wall(wall)) = model.get(o.host).map(|h| &h.kind) else {
            continue;
        };
        // on the wall's centreline, wherever its axis is
        let loc = wall_axes.get(&o.host).copied().unwrap_or(0.0);
        let (pl, shape) = opening_body(&mut c, wall_pl, loc, wall, o);
        // a bare hole is the opening itself, under its own id
        let hole = o.kind == OpeningKind::Hole;
        let (guid, name) = if hole {
            (string(&e.id.ifc_guid()), e.name.clone())
        } else {
            (c.guid(), format!("{} opening", e.name))
        };
        let opening = c.w.add(
            "IFCOPENINGELEMENT",
            format!("{guid},$,{},$,$,{pl},{shape},$,.OPENING.", string(&name)),
        );
        let g = c.guid();
        c.w.add(
            "IFCRELVOIDSELEMENT",
            format!("{g},$,$,$,{wall_r},{opening}"),
        );
        if hole {
            c.phase_of(opening, "IFCOPENINGELEMENT", e);
            continue;
        }

        // the door or window itself: its frame, glass, leaf and handle,
        // each part a box, placed where the opening starts in the wall. A
        // door's own y axis points the way it swings, as IFC has it, so a
        // door opening to the wall's right is placed turned round.
        let cut = creabim_geom::wall::Cut::of(o, wall);
        let start = cut.map_or(DVec3::ZERO, |cut| {
            DVec3::new(cut.u0 - o.offset, 0.0, cut.v0 - o.sill)
        });
        let width = cut.map_or(o.width, |cut| cut.u1 - cut.u0);
        let turned = o.kind == OpeningKind::Door && !o.faces_left(wall);
        let fill_pl = if turned {
            c.local_placement(Some(pl), start + DVec3::X * width, Some(-DVec3::X))
        } else {
            c.local_placement(Some(pl), start, None)
        };
        let mut parts = Vec::new();
        for part in creabim_geom::opening::opening_parts(o, wall) {
            match part {
                creabim_geom::opening::Shaped::Box(b) => {
                    let size = b.hi - b.lo;
                    let lo = if turned {
                        DVec3::new(width - b.hi.x, -b.hi.y, b.lo.z)
                    } else {
                        b.lo
                    };
                    let pos = c.placement3d(lo, None, None);
                    parts.push(c.rect_extrusion(
                        size.truncate() / 2.0,
                        size.x,
                        size.y,
                        pos,
                        size.z,
                    ));
                }
                // shaped in elevation: its outline run across the wall
                creabim_geom::opening::Shaped::Prism(p) => {
                    let (pts, y_far): (Vec<DVec2>, f64) = if turned {
                        (
                            p.outline
                                .iter()
                                .map(|q| DVec2::new(width - q.x, q.y))
                                .collect(),
                            -p.y0,
                        )
                    } else {
                        (p.outline.clone(), p.y1)
                    };
                    let pos =
                        c.placement3d(DVec3::new(0.0, y_far, 0.0), Some(-DVec3::Y), Some(DVec3::X));
                    parts.push(c.profile_extrusion(&pts, pos, p.y1 - p.y0));
                }
                // shaped in section (a louvre's blade): run along the opening
                creabim_geom::opening::Shaped::Slat(p) => {
                    let (pts, x_from): (Vec<DVec2>, f64) = if turned {
                        (
                            p.outline.iter().map(|q| DVec2::new(-q.x, q.y)).collect(),
                            width - p.x1,
                        )
                    } else {
                        (p.outline.clone(), p.x0)
                    };
                    // local Z along the opening, local X across the wall
                    let pos =
                        c.placement3d(DVec3::new(x_from, 0.0, 0.0), Some(DVec3::X), Some(DVec3::Y));
                    parts.push(c.profile_extrusion(&pts, pos, p.x1 - p.x0));
                }
                // shaped in plan (a corner window's mitred member): stood up
                creabim_geom::opening::Shaped::Plan(p) => {
                    let pts: Vec<DVec2> = if turned {
                        p.outline
                            .iter()
                            .map(|q| DVec2::new(width - q.x, -q.y))
                            .collect()
                    } else {
                        p.outline.clone()
                    };
                    let pos = c.placement3d(DVec3::new(0.0, 0.0, p.z0), None, None);
                    parts.push(c.profile_extrusion(&pts, pos, p.z1 - p.z0));
                }
            }
        }
        if parts.is_empty() {
            continue;
        }
        let fill_shape = c.shape("SweptSolid", &parts);
        // hinged on the left seen along its y axis (the way it swings)
        let hinge_left = !o.flip_hand != turned;
        let (class, tail) = match o.kind {
            OpeningKind::Door => (
                "IFCDOOR",
                format!(
                    ",$,{},{},.DOOR.,{}",
                    real(o.height),
                    real(o.width),
                    door_operation(o.frame.operation(o.kind), hinge_left)
                ),
            ),
            OpeningKind::Window => (
                "IFCWINDOW",
                format!(
                    ",$,{},{},.WINDOW.,{}",
                    real(o.height),
                    real(o.width),
                    window_partitioning(&o.frame.grid)
                ),
            ),
            OpeningKind::Hole => continue,
        };
        let fill = product(&mut c, class, e, fill_pl, fill_shape, &tail);
        of_type(e, fill);
        // a grid of cells no IFC operation type says goes out whole, and
        // so do how its leaves are made and its trim
        let mut own = vec![];
        if o.frame.grid.operation(o.kind).is_none()
            || o.frame.grid.operation(o.kind) == Some(creabim_core::Operation::Folding)
        {
            let grid = serde_json::to_string(&o.frame.grid).unwrap_or_default();
            own.push(("CellGrid", crate::params::Prop::Text(grid)));
        }
        let built = &o.frame.construction;
        if !built.is_none() {
            let text = serde_json::to_string(built).unwrap_or_default();
            own.push(("Construction", crate::params::Prop::Text(text)));
        }
        // the ribbon or corner it is in (part 9): a name other programs
        // show, and the whole of it for CreaBIM to read back
        if let Some(a) = &o.arrangement {
            own.push(("Arrangement", crate::params::Prop::Label(a.label())));
            let text = serde_json::to_string(a).unwrap_or_default();
            own.push(("ArrangementData", crate::params::Prop::Text(text)));
        }
        if !own.is_empty() {
            c.pset_on(&[fill], crate::import::OPENING_PSET, &own);
        }
        // its declared ratings go out with every element's, in
        // write_common
        let g = c.guid();
        c.w.add("IFCRELFILLSELEMENT", format!("{g},$,$,$,{opening},{fill}"));
        if let Some((m, _)) = creabim_core::material_of(model, e) {
            by_material.entry(m.id).or_default().push(fill);
        }
        let level = match model.get(o.host).map(|h| &h.kind) {
            Some(ElementKind::Wall(w)) => w.level,
            _ => continue,
        };
        contained.entry(Some(level)).or_default().push(fill);
    }

    // A curtain wall set in a wall cuts its own opening there and fills it,
    // as a door does.
    for e in model.iter() {
        let ElementKind::Wall(wall) = &e.kind else {
            continue;
        };
        for (cw, o) in creabim_core::curtain::voids_in(model, e.id) {
            let (Some(&(wall_r, wall_pl)), Some(&(cw_r, _))) =
                (wall_refs.get(&e.id), wall_refs.get(&cw))
            else {
                continue;
            };
            let loc = wall_axes.get(&e.id).copied().unwrap_or(0.0);
            let (pl, shape) = opening_body(&mut c, wall_pl, loc, wall, &o);
            let name = model.get(cw).map_or("Curtain wall", |x| x.name.as_str());
            let opening = c.w.add(
                "IFCOPENINGELEMENT",
                format!(
                    "{},$,{},$,$,{pl},{shape},$,.OPENING.",
                    c.guid(),
                    string(&format!("{name} opening"))
                ),
            );
            let g = c.guid();
            c.w.add(
                "IFCRELVOIDSELEMENT",
                format!("{g},$,$,$,{wall_r},{opening}"),
            );
            let g = c.guid();
            c.w.add("IFCRELFILLSELEMENT", format!("{g},$,$,$,{opening},{cw_r}"));
        }
    }

    spaces(&mut c, model, &storeys, &wall_refs, &ceilings);

    // Materials, each associated with the elements made of it. Unused ones
    // are written too, so the project's material library goes along.
    let mut material_refs: HashMap<ElementId, Ref> = HashMap::new();
    // each material's surface style, for styling the parts of objects
    let mut material_styles: HashMap<ElementId, Ref> = HashMap::new();
    for e in model.iter() {
        let users = by_material.get(&e.id);
        let ElementKind::Material(mat) = &e.kind else {
            continue;
        };
        let mut description = format!("{HATCH_PREFIX}{}", mat.hatch.label());
        if !mat.description.trim().is_empty() {
            description = format!("{description}. {}", mat.description.trim());
        }
        let category = match mat.category.trim() {
            "" => "$".to_string(),
            c => string(c),
        };
        let m = c.w.add(
            "IFCMATERIAL",
            format!("{},{},{category}", string(&e.name), string(&description)),
        );
        material_refs.insert(e.id, m);
        // the look, as a surface style on the material: colour,
        // transparency, roughness and whether it is a metal
        let [r, g, b] = mat.color.map(|v| real(v as f64));
        let rgb = c.w.add("IFCCOLOURRGB", format!("$,{r},{g},{b}"));
        let method = if mat.metallic >= 0.5 {
            ".METAL."
        } else if mat.category == "glass" {
            ".GLASS."
        } else {
            ".NOTDEFINED."
        };
        let rendering = c.w.add(
            "IFCSURFACESTYLERENDERING",
            format!(
                "{rgb},{},$,$,$,$,$,IFCSPECULARROUGHNESS({}),{method}",
                real((1.0 - mat.opacity as f64).clamp(0.0, 1.0)),
                real(mat.roughness as f64)
            ),
        );
        let style = c.w.add(
            "IFCSURFACESTYLE",
            format!("{},.BOTH.,({rendering})", string(&e.name)),
        );
        material_styles.insert(e.id, style);
        let styled = c.w.add("IFCSTYLEDITEM", format!("$,({style}),$"));
        let styled_rep = c.w.add(
            "IFCSTYLEDREPRESENTATION",
            format!("{},'Style','Material',({styled})", c.body),
        );
        c.w.add(
            "IFCMATERIALDEFINITIONREPRESENTATION",
            format!("$,$,({styled_rep}),{m}"),
        );
        if let Some(users) = users.filter(|u| !u.is_empty()) {
            let g = c.guid();
            c.w.add(
                "IFCRELASSOCIATESMATERIAL",
                format!("{g},$,$,$,{},{m}", list(users)),
            );
        }
    }

    // Assemblies as layer sets, unused ones too; each layered element gets
    // a usage saying how the set lies in it (walls across, slabs down).
    let mut layer_sets: HashMap<ElementId, Ref> = HashMap::new();
    for e in model.iter() {
        let ElementKind::Assembly(a) = &e.kind else {
            continue;
        };
        if a.layers.is_empty() {
            continue;
        }
        let layers: Vec<Ref> = a
            .layers
            .iter()
            .map(|l| {
                let m = l
                    .material
                    .and_then(|m| material_refs.get(&m))
                    .map_or("$".to_string(), |r| r.to_string());
                let air = l.function == creabim_core::LayerFunction::AirGap;
                c.w.add(
                    "IFCMATERIALLAYER",
                    format!(
                        "{m},{},{},{},$,{},$",
                        real(l.thickness),
                        if air { ".T." } else { ".F." },
                        string(l.function.label()),
                        string(l.function.ifc_category())
                    ),
                )
            })
            .collect();
        let description = match a.description.trim() {
            "" => "$".to_string(),
            d => string(d),
        };
        let set = c.w.add(
            "IFCMATERIALLAYERSET",
            format!("{},{},{description}", list(&layers), string(&e.name)),
        );
        layer_sets.insert(e.id, set);
    }
    for (product, assembly, how) in &layered {
        let Some(&set) = layer_sets.get(assembly) else {
            continue;
        };
        let relating = match how {
            LayerUse::Set => set,
            LayerUse::Usage(axis, positive, offset) => c.w.add(
                "IFCMATERIALLAYERSETUSAGE",
                format!(
                    "{set},.{axis}.,{},{},$",
                    if *positive {
                        ".POSITIVE."
                    } else {
                        ".NEGATIVE."
                    },
                    real(*offset)
                ),
            ),
        };
        let g = c.guid();
        c.w.add(
            "IFCRELASSOCIATESMATERIAL",
            format!("{g},$,$,$,({product}),{relating}"),
        );
    }

    // Element types, each with its shared parameters, its material or
    // layer set, and the elements of the type.
    for e in model.iter() {
        let ElementKind::ElementType(t) = &e.kind else {
            continue;
        };
        if let creabim_core::TypeParams::Object(o) = &t.params {
            let looks = Looks {
                materials: &material_refs,
                styles: &material_styles,
            };
            for (level, r) in object_type(&mut c, model, e, t, o, &looks, &storeys) {
                if e.is_site_part() {
                    on_site.push(r);
                    continue;
                }
                contained.entry(Some(level)).or_default().push(r);
            }
            continue;
        }
        let set = c.pset(TYPE_PSET, &type_props(t));
        let description = match t.description.trim() {
            "" => "$".to_string(),
            d => string(d),
        };
        let head = format!(
            "{},$,{},{description},$,({set}),$,$,$",
            string(&e.id.ifc_guid()),
            string(&e.name)
        );
        use creabim_core::TypeParams as P;
        let door_type_tail: String;
        let (class, tail) = match &t.params {
            P::Wall(w) if w.curtain.is_some() => ("IFCCURTAINWALLTYPE", ",.NOTDEFINED."),
            P::Wall(_) => ("IFCWALLTYPE", ",.STANDARD."),
            P::Door(d) => {
                // the type's operation and its user-defined name, then
                // ParameterTakesPrecedence
                let op = door_operation(d.frame.operation(creabim_core::OpeningKind::Door), true);
                let (e, user) = op.split_once(',').unwrap_or((op, "$"));
                door_type_tail = format!(",.DOOR.,{e},.F.,{user}");
                ("IFCDOORTYPE", door_type_tail.as_str())
            }
            P::Window(w) => {
                let (e, user) = window_partitioning(&w.frame.grid)
                    .split_once(',')
                    .map(|(a, b)| (a.to_string(), b.to_string()))
                    .unwrap();
                door_type_tail = format!(",.WINDOW.,{e},.F.,{user}");
                ("IFCWINDOWTYPE", door_type_tail.as_str())
            }
            P::Slab(_) => ("IFCSLABTYPE", ",.FLOOR."),
            P::Roof(_) => ("IFCROOFTYPE", ",.NOTDEFINED."),
            P::Column(_) => ("IFCCOLUMNTYPE", ",.COLUMN."),
            P::Beam(_) => ("IFCBEAMTYPE", ",.BEAM."),
            P::Footing(f) => match f.kind {
                creabim_core::FootingKind::Strip => ("IFCFOOTINGTYPE", ",.STRIP_FOOTING."),
                creabim_core::FootingKind::Pad => ("IFCFOOTINGTYPE", ",.PAD_FOOTING."),
            },
            P::Object(_) => unreachable!("object types are written above"),
        };
        let tr = c.w.add(class, format!("{head}{tail}"));
        let relating = t
            .assembly()
            .and_then(|a| layer_sets.get(&a))
            .or_else(|| t.material().and_then(|m| material_refs.get(&m)))
            .copied();
        if let Some(m) = relating {
            let g = c.guid();
            c.w.add("IFCRELASSOCIATESMATERIAL", format!("{g},$,$,$,({tr}),{m}"));
        }
        if let Some(users) = typed.get(&e.id).filter(|u| !u.is_empty()) {
            let g = c.guid();
            c.w.add(
                "IFCRELDEFINESBYTYPE",
                format!("{g},$,$,$,{},{tr}", list(users)),
            );
        }
    }

    // Grids (wave 3 loose end: none were written). One IfcGrid per grid
    // system and set of levels; each line an IfcGridAxis carrying its
    // label, an arc as its polyline; U the lines across, V those along,
    // W the diagonals; the grid on the lowest level its lines are on, or
    // the building when they are on every level.
    {
        use creabim_core::grids::{self, Direction};
        // (system, levels) -> (name, u, v, w, curves)
        #[allow(clippy::type_complexity)]
        let mut sets: std::collections::BTreeMap<
            (Option<ElementId>, Vec<ElementId>),
            (String, Vec<Ref>, Vec<Ref>, Vec<Ref>, Vec<Ref>),
        > = Default::default();
        for e in model.iter() {
            let ElementKind::Grid(gr) = &e.kind else {
                continue;
            };
            let pts = grids::points(gr);
            if pts.len() < 2 {
                continue;
            }
            let refs: Vec<Ref> = pts
                .iter()
                .map(|p| {
                    c.w.add(
                        "IFCCARTESIANPOINT",
                        format!("({},{})", real(p.x), real(p.y)),
                    )
                })
                .collect();
            let curve = c.w.add("IFCPOLYLINE", list(&refs));
            let axis = c.w.add(
                "IFCGRIDAXIS",
                format!("{},{curve},.T.", string(&grids::display_label(model, gr))),
            );
            let system = grids::group_of(model, gr).map(|(ge, _)| (ge.id, ge.name.clone()));
            let mut levels = gr.levels.clone();
            levels.sort();
            let key = (system.as_ref().map(|s| s.0), levels.clone());
            let name = match (&system, levels.is_empty()) {
                (Some((_, n)), true) => n.clone(),
                (None, true) => "Grid".to_string(),
                (s, false) => {
                    let names: Vec<String> = levels
                        .iter()
                        .filter_map(|l| model.get(*l).map(|x| x.name.clone()))
                        .collect();
                    format!(
                        "{} ({})",
                        s.as_ref().map_or("Grid".to_string(), |s| s.1.clone()),
                        names.join(", ")
                    )
                }
            };
            let entry = sets
                .entry(key)
                .or_insert_with(|| (name, vec![], vec![], vec![], vec![]));
            // the 3D curve for the footprint a viewer draws
            let refs3: Vec<Ref> = pts
                .iter()
                .map(|p| c.w.add("IFCCARTESIANPOINT", point3(p.x, p.y, 0.0)))
                .collect();
            entry.4.push(c.w.add("IFCPOLYLINE", list(&refs3)));
            match grids::direction_of(model, gr) {
                Direction::Across | Direction::Spoke => entry.1.push(axis),
                Direction::Along | Direction::Ring => entry.2.push(axis),
                Direction::Diagonal => entry.3.push(axis),
            }
        }
        for ((system, levels), (name, mut u, mut v, w, curves)) in sets {
            let _ = system;
            // IFC asks for at least one U and one V axis
            if v.is_empty() && u.len() > 1 {
                v.push(u.pop().expect("more than one"));
            } else if u.is_empty() && v.len() > 1 {
                u.push(v.pop().expect("more than one"));
            }
            if u.is_empty() || v.is_empty() {
                // a single line: kept as one U and the same as V would be
                // invalid; it goes out as an annotation-free grid of one
                u.append(&mut v);
                v = u.clone();
            }
            let lowest = levels
                .iter()
                .filter_map(|l| model.level(*l).map(|x| (*l, x.elevation)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|x| x.0);
            let rel = lowest
                .and_then(|l| storeys.get(&l))
                .map_or(bldg_pl, |s| s.1);
            let pl = c.local_placement(Some(rel), DVec3::ZERO, None);
            let set = c.w.add("IFCGEOMETRICCURVESET", list(&curves));
            let rep = c.w.add(
                "IFCSHAPEREPRESENTATION",
                format!("{},'FootPrint','GeometricCurveSet',({set})", c.footprint),
            );
            let shape = c.w.add("IFCPRODUCTDEFINITIONSHAPE", format!("$,$,({rep})"));
            let ws = if w.is_empty() {
                "$".to_string()
            } else {
                list(&w)
            };
            let grid = c.w.add(
                "IFCGRID",
                format!(
                    "{},$,{},$,$,{pl},{shape},{},{},{ws},.{}.",
                    c.guid(),
                    string(&name),
                    list(&u),
                    list(&v),
                    if w.is_empty() {
                        "RECTANGULAR"
                    } else {
                        "IRREGULAR"
                    }
                ),
            );
            contained.entry(lowest).or_default().push(grid);
        }
    }

    // Spatial structure.
    let g = c.guid();
    c.w.add("IFCRELAGGREGATES", format!("{g},$,$,$,{project},({site})"));
    let g = c.guid();
    c.w.add("IFCRELAGGREGATES", format!("{g},$,$,$,{site},({building})"));
    if !storey_order.is_empty() {
        let g = c.guid();
        c.w.add(
            "IFCRELAGGREGATES",
            format!("{g},$,$,$,{building},{}", list(&storey_order)),
        );
    }
    let mut keys: Vec<_> = contained.keys().copied().collect();
    keys.sort_by_key(|k| {
        k.map(|id| storeys.get(&id).map(|s| s.0 .0))
            .unwrap_or(Some(0))
    });
    for k in keys {
        let items = &contained[&k];
        let container = k
            .and_then(|id| storeys.get(&id))
            .map(|s| s.0)
            .unwrap_or(building);
        let g = c.guid();
        c.w.add(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE",
            format!("{g},$,$,$,{},{container}", list(items)),
        );
    }

    if !on_site.is_empty() {
        let g = c.guid();
        c.w.add(
            "IFCRELCONTAINEDINSPATIALSTRUCTURE",
            format!("{g},$,$,$,{},{site}", list(&on_site)),
        );
    }

    // the project's groups, as they came: each with what it holds
    let wanted: std::collections::HashSet<String> = groups
        .iter()
        .flat_map(|(_, m)| m.iter().map(|id| string(&id.ifc_guid())))
        .collect();
    let written = c.w.objects_with(&wanted);
    for (g, members) in &groups {
        let src = g.ifc.clone().unwrap_or_default();
        let guid = if src.guid.is_empty() {
            c.guid()
        } else {
            string(&src.guid)
        };
        let opt = |v: &Option<String>| v.as_deref().map_or_else(|| "$".to_string(), string);
        let group = c.w.add(
            "IFCGROUP",
            format!(
                "{guid},$,{},{},{}",
                string(&g.name),
                opt(&src.description),
                opt(&src.object_type)
            ),
        );
        let refs: Vec<Ref> = members
            .iter()
            .filter_map(|m| written.get(&string(&m.ifc_guid())).copied())
            .collect();
        if !refs.is_empty() {
            let rg = c.guid();
            c.w.add(
                "IFCRELASSIGNSTOGROUP",
                format!("{rg},$,$,$,{},$,{group}", list(&refs)),
            );
        }
    }

    // base quantities, worked out as the take-off works them out
    write_quantities(&mut c, model);
    // the common sets: declared ratings, load-bearing, external
    write_common(&mut c, model);
    // the documents given in CreaBIM (Element documents REV00), each one
    // reference shared by what has it; the file's own go out with the rest
    // kept below
    write_documents(&mut c, model);
    // the property sets and classifications kept from the file read,
    // back on the objects still here
    write_kept(&mut c, model);
    // the relationships and objects kept from that file: types, assemblies,
    // openings, materials, drawings and documents CreaBIM doesn't model
    crate::export_kept::write(
        &mut c.w,
        model,
        &crate::export_kept::Anchors {
            project,
            site,
            building,
            model: model_ctx,
            body: c.body,
            axis: c.axis,
            plan: c.footprint,
        },
    );

    tick(total, total, "Writing the file");
    let header = format!(
        "FILE_DESCRIPTION(('ViewDefinition [DesignTransferView]'),'2;1');\n\
         FILE_NAME({},{},(''),(''),{},{},'');\n\
         FILE_SCHEMA(('IFC4'));\n",
        string(&format!("{}.ifc", opts.project_name)),
        string(&opts.timestamp),
        string(concat!("CreaBIM ", env!("CARGO_PKG_VERSION"))),
        string("CreaBIM"),
    );
    c.w.finish(&header)
}

/// The materials written, and their surface styles.
struct Looks<'a> {
    materials: &'a HashMap<ElementId, Ref>,
    styles: &'a HashMap<ElementId, Ref>,
}

/// An object type: its parts as one representation map, each part's face
/// set styled with its material's look, its plan symbol as another, its
/// materials as a constituent set; then every object of the type, each
/// mapping both maps where it stands. Returns the objects written, with
/// their levels.
fn object_type(
    c: &mut Ctx,
    model: &Model,
    e: &Element,
    t: &creabim_core::ElementType,
    o: &creabim_core::ObjectType,
    looks: &Looks,
    storeys: &HashMap<ElementId, (Ref, Ref)>,
) -> Vec<(ElementId, Ref)> {
    let kind = o.ifc.normalized();
    // the parts, in the type's own axes
    let mut items = vec![];
    for p in &o.parts {
        let coords: Vec<String> = p
            .mesh
            .vertices
            .iter()
            .map(|v| point3(v.x, v.y, v.z))
            .collect();
        let pts =
            c.w.add("IFCCARTESIANPOINTLIST3D", format!("({})", coords.join(",")));
        let mut tris = vec![];
        for f in 0..p.mesh.faces.len() {
            for [a, b, d] in p.mesh.face_triangles(f) {
                tris.push(format!("({},{},{})", a + 1, b + 1, d + 1));
            }
        }
        let fs = c.w.add(
            "IFCTRIANGULATEDFACESET",
            format!("{pts},$,.T.,({}),$", tris.join(",")),
        );
        if let Some(style) = p.material.and_then(|m| looks.styles.get(&m)) {
            c.w.add("IFCSTYLEDITEM", format!("{fs},({style}),$"));
        }
        items.push(fs);
    }
    let body_rep = c.body_rep("Tessellation", &items);
    let origin = c.placement3d(DVec3::ZERO, None, None);
    let body_map =
        c.w.add("IFCREPRESENTATIONMAP", format!("{origin},{body_rep}"));
    // the plan symbol, as polylines on the floor
    let mut curves = vec![];
    for s in o.plan_symbol() {
        let pts: Vec<DVec2> = match s {
            creabim_core::objects::SymbolItem::Loop(mut p) => {
                if let Some(first) = p.first().copied() {
                    p.push(first);
                }
                p
            }
            creabim_core::objects::SymbolItem::Line(a, b) => vec![a, b],
        };
        if pts.len() < 2 {
            continue;
        }
        let refs: Vec<Ref> = pts
            .iter()
            .map(|p| c.w.add("IFCCARTESIANPOINT", point3(p.x, p.y, 0.0)))
            .collect();
        curves.push(c.w.add("IFCPOLYLINE", list(&refs)));
    }
    let foot_map = (!curves.is_empty()).then(|| {
        let set = c.w.add("IFCGEOMETRICCURVESET", list(&curves));
        let rep = c.w.add(
            "IFCSHAPEREPRESENTATION",
            format!("{},'FootPrint','GeometricCurveSet',({set})", c.footprint),
        );
        let origin = c.placement3d(DVec3::ZERO, None, None);
        c.w.add("IFCREPRESENTATIONMAP", format!("{origin},{rep}"))
    });
    let maps: Vec<Ref> = std::iter::once(body_map).chain(foot_map).collect();
    let set = c.pset(TYPE_PSET, &type_props(t));
    let description = match t.description.trim() {
        "" => "$".to_string(),
        d => string(d),
    };
    let type_class = kind.type_class().to_ascii_uppercase();
    // AssemblyPlace comes before PredefinedType on furniture types
    let place = if type_class == "IFCFURNITURETYPE" {
        ".NOTDEFINED.,"
    } else {
        ""
    };
    let tr = c.w.add(
        &type_class,
        format!(
            "{},$,{},{description},$,({set}),{},$,{},{place}.{}.",
            string(&e.id.ifc_guid()),
            string(&e.name),
            list(&maps),
            string(&e.name),
            kind.predefined
        ),
    );
    // the materials of the parts
    let mut constituents = vec![];
    for p in &o.parts {
        let Some(m) = p.material.and_then(|m| looks.materials.get(&m)) else {
            continue;
        };
        let name = if p.name.is_empty() {
            "$".to_string()
        } else {
            string(&p.name)
        };
        constituents.push(c.w.add("IFCMATERIALCONSTITUENT", format!("{name},$,{m},$,$")));
    }
    if !constituents.is_empty() {
        let cs = c.w.add(
            "IFCMATERIALCONSTITUENTSET",
            format!("{},$,{}", string(&e.name), list(&constituents)),
        );
        let g = c.guid();
        c.w.add("IFCRELASSOCIATESMATERIAL", format!("{g},$,$,$,({tr}),{cs}"));
    }
    // the objects of the type
    let mut out = vec![];
    for id in creabim_core::types::instances(model, e.id) {
        let Some(oe) = model.get(id) else { continue };
        let ElementKind::Object(ob) = &oe.kind else {
            continue;
        };
        let Some(&(_, spl)) = storeys.get(&ob.level) else {
            continue;
        };
        let (s, co) = ob.rotation.to_radians().sin_cos();
        // hung from the ceiling or not, where it stands above its storey
        let floor = model.level(ob.level).map_or(0.0, |l| l.elevation);
        let z = creabim_core::objects::base_z_of(model, ob, o) - floor;
        let pl = c.local_placement(
            Some(spl),
            ob.position.extend(z),
            Some(DVec3::new(co, s, 0.0)),
        );
        let mut reps = vec![];
        for (i, map) in maps.iter().enumerate() {
            let at = c.w.add("IFCCARTESIANPOINT", "(0.,0.,0.)");
            let op = c.w.add(
                "IFCCARTESIANTRANSFORMATIONOPERATOR3D",
                format!("$,$,{at},$,$"),
            );
            let item = c.w.add("IFCMAPPEDITEM", format!("{map},{op}"));
            let (ctx, ident) = if i == 0 {
                (c.body, "Body")
            } else {
                (c.footprint, "FootPrint")
            };
            reps.push(c.w.add(
                "IFCSHAPEREPRESENTATION",
                format!("{ctx},'{ident}','MappedRepresentation',({item})"),
            ));
        }
        let shape =
            c.w.add("IFCPRODUCTDEFINITIONSHAPE", format!("$,$,{}", list(&reps)));
        let object_type = if kind.predefined == "USERDEFINED" {
            string(&e.name)
        } else {
            "$".into()
        };
        let r = c.w.add(
            &kind.class.to_ascii_uppercase(),
            format!(
                "{},$,{},$,{object_type},{pl},{shape},{},.{}.",
                string(&oe.id.ifc_guid()),
                string(&oe.name),
                c.tag(oe.id),
                kind.predefined
            ),
        );
        c.phase_of(r, &kind.class.to_ascii_uppercase(), oe);
        out.push((ob.level, r));
    }
    if !out.is_empty() {
        let g = c.guid();
        let users: Vec<Ref> = out.iter().map(|o| o.1).collect();
        c.w.add(
            "IFCRELDEFINESBYTYPE",
            format!("{g},$,$,$,{},{tr}", list(&users)),
        );
    }
    out
}

/// Whether element `e`'s layers are not all as its layer set puts them:
/// a layer end or edge moved by hand, or cores lapped at a corner.
fn layers_differ(e: &Element, joins: &HashMap<ElementId, creabim_core::WallEnds>) -> bool {
    match &e.kind {
        ElementKind::Wall(w) => {
            !w.layer_ends.is_empty()
                || joins
                    .get(&e.id)
                    .is_some_and(|j| j.core_line.iter().any(Option::is_some))
        }
        ElementKind::Slab(s) => !s.layer_edges.is_empty(),
        ElementKind::Roof(r) => !r.layer_edges.is_empty(),
        _ => false,
    }
}

/// Element `host`'s layers as `IfcBuildingElementPart`s that make it up,
/// each its own solid with its layer's material, where the layers are not
/// all as the layer set puts them; the host keeps its body and layer set
/// for tools that read only those. Placed in the storey at `spl`, `z0` up.
#[allow(clippy::too_many_arguments)]
fn write_layer_parts(
    c: &mut Ctx,
    model: &Model,
    e: &Element,
    host: Ref,
    spl: Ref,
    z0: f64,
    joins: &HashMap<ElementId, creabim_core::WallEnds>,
    by_material: &mut HashMap<ElementId, Vec<Ref>>,
    as_coverings: bool,
) {
    if as_coverings {
        write_layer_coverings(c, model, e, host, spl, z0, joins, by_material);
        return;
    }
    if !layers_differ(e, joins) {
        return;
    }
    let names: Vec<String> = creabim_core::assembly_of(model, e)
        .map(|(_, a)| {
            a.layers
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    l.material
                        .and_then(|m| model.get(m))
                        .map_or_else(|| format!("layer {}", i + 1), |m| m.name.clone())
                })
                .collect()
        })
        .unwrap_or_default();
    let mut parts = vec![];
    for (layer, mesh) in creabim_geom::scene::layer_parts(model, e, joins) {
        if mesh.indices.is_empty() {
            continue;
        }
        let pl = c.local_placement(Some(spl), DVec3::ZERO, None);
        let fs = tessellate(c, &mesh, |p| p - DVec3::Z * z0);
        let shape = c.shape("Tessellation", &[fs]);
        let g = c.guid();
        let name = names
            .get(layer.index)
            .cloned()
            .unwrap_or_else(|| format!("layer {}", layer.index + 1));
        let p = c.w.add(
            "IFCBUILDINGELEMENTPART",
            format!(
                "{g},$,{},$,$,{pl},{shape},$,.NOTDEFINED.",
                string(&format!("{} {name}", e.name))
            ),
        );
        if let Some(m) = layer.material {
            by_material.entry(m).or_default().push(p);
        }
        parts.push(p);
    }
    if !parts.is_empty() {
        let g = c.guid();
        c.w.add(
            "IFCRELAGGREGATES",
            format!("{g},$,$,$,{host},{}", list(&parts)),
        );
    }
}

/// Element `host`'s layers other than its structure as `IfcCovering`s
/// covering it (MU20, "Layers as separate elements"): finishes as
/// cladding, flooring or roofing, insulation, membranes; air gaps are
/// left out. The host's own body is its structure.
#[allow(clippy::too_many_arguments)]
fn write_layer_coverings(
    c: &mut Ctx,
    model: &Model,
    e: &Element,
    host: Ref,
    spl: Ref,
    z0: f64,
    joins: &HashMap<ElementId, creabim_core::WallEnds>,
    by_material: &mut HashMap<ElementId, Vec<Ref>>,
) {
    use creabim_core::LayerFunction as F;
    let parts = creabim_geom::scene::layer_parts(model, e, joins);
    if parts.len() < 2 {
        return;
    }
    let mut covers = vec![];
    for (layer, mesh) in parts {
        if layer.core || mesh.indices.is_empty() || layer.function == F::AirGap {
            continue;
        }
        let kind = match (layer.function, &e.kind) {
            (F::Insulation, _) => "INSULATION",
            (F::Membrane, _) => "MEMBRANE",
            (_, ElementKind::Slab(_)) => "FLOORING",
            (_, ElementKind::Roof(_)) => "ROOFING",
            _ => "CLADDING",
        };
        let name = layer
            .material
            .and_then(|m| model.get(m))
            .map_or_else(|| format!("layer {}", layer.index + 1), |m| m.name.clone());
        let pl = c.local_placement(Some(spl), DVec3::ZERO, None);
        let fs = tessellate(c, &mesh, |p| p - DVec3::Z * z0);
        let shape = c.shape("Tessellation", &[fs]);
        let g = c.guid();
        let r = c.w.add(
            "IFCCOVERING",
            format!(
                "{g},$,{},$,$,{pl},{shape},$,.{kind}.",
                string(&format!("{} {name}", e.name))
            ),
        );
        if let Some(m) = layer.material {
            by_material.entry(m).or_default().push(r);
        }
        covers.push(r);
    }
    if !covers.is_empty() {
        let g = c.guid();
        c.w.add(
            "IFCRELCOVERSBLDGELEMENTS",
            format!("{g},$,$,$,{host},{}", list(&covers)),
        );
    }
}

/// How a layered element takes its assembly's layer set.
enum LayerUse {
    /// Through an `IfcMaterialLayerSetUsage`: the axis the layers run
    /// along, whether they run its positive way, and where the first
    /// layer starts from the element's reference line.
    Usage(&'static str, bool, f64),
    /// The set itself, for shapes that aren't straight extrusions.
    Set,
}

/// A triangle mesh as an `IfcTriangulatedFaceSet`, its corners taken into
/// the product's own coordinates by `to_local`.
/// `solid`, in a wall's own axes (`x` along `d` and `y` along `n` from
/// `origin`, `z` up from `z0`), with what lies above the underside `f` cut
/// away: a half-space over the face's plane, bounded by its outline in
/// plan, as Revit writes walls attached to roofs.
#[allow(clippy::too_many_arguments)]
fn clip_under(
    c: &mut Ctx,
    solid: Ref,
    f: &creabim_core::walls::TopFace,
    to_local: impl Fn(DVec2) -> DVec2,
    d: DVec2,
    n: DVec2,
    origin: DVec2,
    z0: f64,
) -> Ref {
    // the plane z = h + a x + b y in the wall's axes
    let (a, b) = (f.g.dot(d), f.g.dot(n));
    let h = f.c + f.g.dot(origin) - z0;
    let normal = DVec3::new(-a, -b, 1.0).normalize();
    let along = DVec3::new(1.0, 0.0, a).normalize();
    let pos = c.placement3d(DVec3::new(0.0, 0.0, h), Some(normal), Some(along));
    let plane = c.w.add("IFCPLANE", pos.to_string());
    let mut pts: Vec<Ref> = f
        .outline
        .iter()
        .map(|p| {
            let q = to_local(*p);
            c.w.add("IFCCARTESIANPOINT", point2(q.x, q.y))
        })
        .collect();
    pts.push(pts[0]);
    let boundary = c.w.add("IFCPOLYLINE", list(&pts));
    let at = c.placement3d(DVec3::ZERO, None, None);
    // the material is above the plane, on the side its normal points to
    let half = c.w.add(
        "IFCPOLYGONALBOUNDEDHALFSPACE",
        format!("{plane},.F.,{at},{boundary}"),
    );
    c.w.add(
        "IFCBOOLEANCLIPPINGRESULT",
        format!(".DIFFERENCE.,{solid},{half}"),
    )
}

fn tessellate(
    c: &mut Ctx,
    m: &creabim_geom::mesh::TriMesh,
    to_local: impl Fn(DVec3) -> DVec3,
) -> Ref {
    let coords: Vec<String> = m
        .positions
        .iter()
        .map(|p| {
            let q = to_local(glam::Vec3::from(*p).as_dvec3());
            point3(q.x, q.y, q.z)
        })
        .collect();
    let pts =
        c.w.add("IFCCARTESIANPOINTLIST3D", format!("({})", coords.join(",")));
    let tris: Vec<String> = m
        .indices
        .chunks(3)
        .map(|t| format!("({},{},{})", t[0] + 1, t[1] + 1, t[2] + 1))
        .collect();
    c.w.add(
        "IFCTRIANGULATEDFACESET",
        format!("{pts},$,.T.,({}),$", tris.join(",")),
    )
}

/// The placement and body of opening `o` in `wall` (placed at `wall_pl`,
/// its axis `loc` off its centreline): a box through the wall, a little
/// proud of both faces.
fn opening_body(
    c: &mut Ctx,
    wall_pl: Ref,
    loc: f64,
    wall: &creabim_core::Wall,
    o: &creabim_core::Opening,
) -> (Ref, Ref) {
    let eps = 0.05;
    let pl = c.local_placement(Some(wall_pl), DVec3::new(o.offset, -loc, o.sill), None);
    let pos = c.placement3d(
        DVec3::new(o.width / 2.0, wall.thickness / 2.0 + eps, o.height / 2.0),
        Some(-DVec3::Y),
        Some(DVec3::X),
    );
    let depth = wall.thickness + 2.0 * eps;
    let outline = o.frame.grid.outline.fitted(o.width, o.height);
    let solid = if outline.is_rectangle() {
        c.rect_extrusion(DVec2::ZERO, o.width, o.height, pos, depth)
    } else {
        // an arched or raked head: its outline, about the middle
        let mid = DVec2::new(o.width / 2.0, o.height / 2.0);
        let mut pts = vec![DVec2::ZERO - mid, DVec2::new(o.width, 0.0) - mid];
        pts.extend(
            outline
                .head(o.width, o.height, 0.0)
                .into_iter()
                .rev()
                .map(|(x, z)| DVec2::new(x, z) - mid),
        );
        c.profile_extrusion(&pts, pos, depth)
    };
    (pl, c.shape("SweptSolid", &[solid]))
}

/// Rooms as IfcSpace: each its outline extruded from its floor to its
/// top, numbered in Name and named in LongName, with its finishes,
/// occupancy and quantities, bounded (IfcRelSpaceBoundary) by the walls
/// round it and covered (IfcRelCoversSpaces) by its ceilings; room
/// separators as the IfcVirtualElements that bound rooms where there is
/// no wall. Spaces make up their storeys (IfcRelAggregates).
fn spaces(
    c: &mut Ctx,
    model: &Model,
    storeys: &HashMap<ElementId, (Ref, Ref)>,
    wall_refs: &HashMap<ElementId, (Ref, Ref)>,
    ceilings: &[(ElementId, Ref)],
) {
    let mut separators: HashMap<ElementId, Ref> = HashMap::new();
    let mut contained: HashMap<ElementId, Vec<Ref>> = HashMap::new();
    for e in model.iter() {
        let ElementKind::RoomSeparator(s) = &e.kind else {
            continue;
        };
        let Some(&(_, spl)) = storeys.get(&s.level) else {
            continue;
        };
        let pl = c.local_placement(Some(spl), DVec3::ZERO, None);
        let a = c.w.add("IFCCARTESIANPOINT", point2(s.a.x, s.a.y));
        let b = c.w.add("IFCCARTESIANPOINT", point2(s.b.x, s.b.y));
        let line = c.w.add("IFCPOLYLINE", list([a, b]));
        let axis = c.w.add(
            "IFCSHAPEREPRESENTATION",
            format!("{},'Axis','Curve2D',({line})", c.axis),
        );
        let shape =
            c.w.add("IFCPRODUCTDEFINITIONSHAPE", format!("$,$,({axis})"));
        let r = product(c, "IFCVIRTUALELEMENT", e, pl, shape, ",$");
        separators.insert(e.id, r);
        contained.entry(s.level).or_default().push(r);
    }
    for (level, items) in contained {
        if let Some(&(storey, _)) = storeys.get(&level) {
            let g = c.guid();
            c.w.add(
                "IFCRELCONTAINEDINSPATIALSTRUCTURE",
                format!("{g},$,$,$,{},{storey}", list(&items)),
            );
        }
    }
    // how many rooms each wall bounds: one is an outside wall
    let shapes: Vec<(&Element, &creabim_core::Room, creabim_core::room::RoomShape)> = model
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Room(r) => Some((e, r, creabim_core::room::shape_of(model, r)?)),
            _ => None,
        })
        .collect();
    let mut bounding: HashMap<ElementId, usize> = HashMap::new();
    for (_, _, s) in &shapes {
        let mut seen: Vec<ElementId> = s.bounded_by.iter().flatten().copied().collect();
        seen.sort_by_key(|id| id.0);
        seen.dedup();
        for id in seen {
            *bounding.entry(id).or_default() += 1;
        }
    }
    let mut by_storey: HashMap<ElementId, Vec<Ref>> = HashMap::new();
    for (e, r, s) in &shapes {
        let Some(&(_, spl)) = storeys.get(&r.level) else {
            continue;
        };
        let height = creabim_core::room::height(model, r);
        let pl = c.local_placement(Some(spl), DVec3::ZERO, None);
        let solid = c.plan_extrusion(&s.outline, 0.0, height);
        let shape = c.shape("SweptSolid", &[solid]);
        let number = if r.number.is_empty() {
            e.name.clone()
        } else {
            r.number.clone()
        };
        let space = c.w.add(
            "IFCSPACE",
            format!(
                "{},$,{},$,$,{pl},{shape},{},.ELEMENT.,.SPACE.,$",
                string(&e.id.ifc_guid()),
                string(&number),
                string(&e.name)
            ),
        );
        c.phase_of(space, "IFCSPACE", e);
        by_storey.entry(r.level).or_default().push(space);
        let outdoor = [&r.occupancy, &r.department]
            .iter()
            .any(|s| s.eq_ignore_ascii_case("outdoor"));
        let area = creabim_core::room::net_area(model, r, s);
        c.pset_on(
            &[space],
            "Pset_SpaceCommon",
            &[
                ("Reference", Prop::Label(number.clone())),
                ("IsExternal", Prop::Bool(outdoor)),
            ],
        );
        let finishes: Vec<(&'static str, Prop)> = [
            ("FloorCovering", &r.floor_finish),
            ("WallCovering", &r.wall_finish),
            ("CeilingCovering", &r.ceiling_finish),
        ]
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| (k, Prop::Label(v.clone())))
        .collect();
        if !finishes.is_empty() {
            c.pset_on(&[space], "Pset_SpaceCoveringRequirements", &finishes);
        }
        if !r.occupancy.is_empty() {
            c.pset_on(
                &[space],
                "Pset_SpaceOccupancyRequirements",
                &[("OccupancyType", Prop::Label(r.occupancy.clone()))],
            );
        }
        c.pset_on(&[space], ROOM_PSET, &room_props(r));
        // its quantities, for take-off in other tools
        let q = [
            c.w.add(
                "IFCQUANTITYAREA",
                format!("'NetFloorArea',$,$,{},$", real(area)),
            ),
            c.w.add(
                "IFCQUANTITYLENGTH",
                format!("'GrossPerimeter',$,$,{},$", real(s.perimeter())),
            ),
            c.w.add(
                "IFCQUANTITYLENGTH",
                format!("'Height',$,$,{},$", real(height)),
            ),
        ];
        let g = c.guid();
        let quantities = c.w.add(
            "IFCELEMENTQUANTITY",
            format!("{g},$,'Qto_SpaceBaseQuantities',$,$,{}", list(q)),
        );
        let g = c.guid();
        c.w.add(
            "IFCRELDEFINESBYPROPERTIES",
            format!("{g},$,$,$,({space}),{quantities}"),
        );
        // what bounds it, once each
        let mut done: Vec<ElementId> = vec![];
        for id in s.bounded_by.iter().flatten() {
            if done.contains(id) {
                continue;
            }
            done.push(*id);
            let (element, physical) = match (wall_refs.get(id), separators.get(id)) {
                (Some(&(w, _)), _) => (w, true),
                (None, Some(&v)) => (v, false),
                _ => continue,
            };
            let side = if !physical || bounding.get(id).copied().unwrap_or(0) > 1 {
                ".INTERNAL."
            } else {
                ".EXTERNAL."
            };
            let kind = if physical { ".PHYSICAL." } else { ".VIRTUAL." };
            let g = c.guid();
            c.w.add(
                "IFCRELSPACEBOUNDARY",
                format!("{g},$,$,$,{space},{element},$,{kind},{side}"),
            );
        }
        let covering: Vec<Ref> = ceilings
            .iter()
            .filter(|(room, _)| *room == e.id)
            .map(|c| c.1)
            .collect();
        if !covering.is_empty() {
            let g = c.guid();
            c.w.add(
                "IFCRELCOVERSSPACES",
                format!("{g},$,$,$,{space},{}", list(&covering)),
            );
        }
    }
    let mut levels: Vec<ElementId> = by_storey.keys().copied().collect();
    levels.sort_by_key(|l| storeys.get(l).map(|s| s.0 .0));
    for level in levels {
        let (storey, _) = storeys[&level];
        let g = c.guid();
        c.w.add(
            "IFCRELAGGREGATES",
            format!("{g},$,$,$,{storey},{}", list(&by_storey[&level])),
        );
    }
}

fn product(c: &mut Ctx, class: &str, e: &Element, pl: Ref, shape: Ref, tail: &str) -> Ref {
    // the tail starts with the Tag, unset: the element's mark goes there
    let tagged;
    let tail = match tail.strip_prefix(",$") {
        Some(rest) if c.marks.contains_key(&e.id) => {
            tagged = format!(",{}{rest}", c.tag(e.id));
            tagged.as_str()
        }
        _ => tail,
    };
    let r = c.w.add(
        class,
        format!(
            "{},$,{},$,$,{pl},{shape}{tail}",
            string(&e.id.ifc_guid()),
            string(&e.name)
        ),
    );
    c.products.insert(e.id, r);
    if !c.phases.is_empty() && creabim_core::phases::phased(e) {
        c.phasing(r, class, e.phase);
    }
    r
}

/// `model` without the copies made for options that are unchanged while
/// the piece they stand for is still there (see
/// [`creabim_core::groups::make_options`]): the file holds that piece
/// once, in every group that held it. `None` when there are none.
fn fold_copies(model: &Model) -> Option<Model> {
    let fold: Vec<ElementId> = creabim_core::groups::copies(model)
        .iter()
        .filter(|c| creabim_core::groups::stands_for(model, c.copy) == c.of)
        .map(|c| c.copy)
        .collect();
    if fold.is_empty() {
        return None;
    }
    let mut doc = creabim_core::Document::from_model(model.clone());
    let tx = creabim_core::Transaction::new("Fold option copies").remove_all_cascade(model, &fold);
    doc.commit(tx).ok()?;
    Some(doc.model().clone())
}

/// The STEP entity and record tail of an extrusion, a sweep or a site
/// element: the class picked for it, or a proxy when the exporter can't
/// write that class.
fn part_entity(e: &Element) -> (String, String) {
    match &e.kind {
        ElementKind::Site(s) if matches!(s.form, creabim_core::SiteForm::Terrain { .. }) => {
            ("IFCGEOGRAPHICELEMENT".into(), ",$,.TERRAIN.".into())
        }
        _ => free_mesh_entity(e.ifc_class()),
    }
}

/// The marks of everything that has one, objects' from their types.
fn element_marks(model: &Model) -> HashMap<ElementId, String> {
    let mut out = creabim_core::marks(model);
    for e in model.iter() {
        if matches!(e.kind, ElementKind::Object(_)) {
            if let Some(m) = creabim_core::marks::object_mark(model, e) {
                out.insert(e.id, m);
            }
        }
    }
    out
}

/// The STEP entity and record tail (Tag, PredefinedType) of a member.
fn member_entity(model: &Model, k: &ElementKind) -> (&'static str, String) {
    if let ElementKind::Stair(s) = k {
        // its shape names the type: straight, quarter turn, half turn ...
        let class = if s.is_ramp() { "IFCRAMP" } else { "IFCSTAIR" };
        return (class, format!(",$,.{}.", s.ifc_type(s.rise(model))));
    }
    let (class, tail) = match k {
        ElementKind::Column(_) => ("IFCCOLUMN", ",$,.COLUMN."),
        ElementKind::Beam(_) => ("IFCBEAM", ",$,.BEAM."),
        ElementKind::Roof(r) => (
            "IFCROOF",
            match r.effective_shape() {
                creabim_core::RoofShape::Flat => ",$,.FLAT_ROOF.",
                creabim_core::RoofShape::Shed => ",$,.SHED_ROOF.",
                creabim_core::RoofShape::Gable => ",$,.GABLE_ROOF.",
                creabim_core::RoofShape::Hip => ",$,.HIP_ROOF.",
            },
        ),
        ElementKind::Footing(f) => match f.kind {
            creabim_core::FootingKind::Strip => ("IFCFOOTING", ",$,.STRIP_FOOTING."),
            creabim_core::FootingKind::Pad => ("IFCFOOTING", ",$,.PAD_FOOTING."),
        },
        ElementKind::Railing(r) if r.on_wall => ("IFCRAILING", ",$,.HANDRAIL."),
        _ => ("IFCRAILING", ",$,.GUARDRAIL."),
    };
    (class, tail.to_string())
}

/// The STEP entity and record tail for a free mesh's IFC class; classes the
/// exporter cannot write become proxies.
fn free_mesh_entity(class: &str) -> (String, String) {
    match crate::classes::ifc4_element_tail(class) {
        Some(tail) => (class.to_ascii_uppercase(), format!(",${tail}")),
        None => ("IFCBUILDINGELEMENTPROXY".into(), ",$,.NOTDEFINED.".into()),
    }
}

/// A door's IFC4 operation type and its user-defined operation (the
/// last two attributes of an IfcDoor), from its operation and whether it
/// is hinged on (slides to) its left seen along its y axis.
fn door_operation(op: creabim_core::Operation, left: bool) -> &'static str {
    use creabim_core::Operation as O;
    match op {
        O::Standard if left => ".SINGLE_SWING_LEFT.,$",
        O::Standard => ".SINGLE_SWING_RIGHT.,$",
        O::DoubleSwing => ".DOUBLE_DOOR_SINGLE_SWING.,$",
        // a slider is built sliding towards its hinge side
        // IFC has no pocket door: it goes out as the slider it is
        O::SurfaceSliding | O::Pocket if left => ".SLIDING_TO_LEFT.,$",
        O::SurfaceSliding | O::Pocket => ".SLIDING_TO_RIGHT.,$",
        O::SurfaceSlidingPair | O::PocketPair => ".DOUBLE_DOOR_SLIDING.,$",
        O::MultiSlide => ".USERDEFINED.,'MULTI_PANEL_SLIDING'",
        O::DoubleAction if left => ".DOUBLE_SWING_LEFT.,$",
        O::DoubleAction => ".DOUBLE_SWING_RIGHT.,$",
        O::DoubleActionPair => ".DOUBLE_DOOR_DOUBLE_SWING.,$",
        // folding towards the hinge side; how many panels goes out in
        // the cell grid
        O::Bifold | O::Folding if left => ".FOLDING_TO_LEFT.,$",
        O::Bifold | O::Folding => ".FOLDING_TO_RIGHT.,$",
        O::BifoldPair => ".DOUBLE_DOOR_FOLDING.,$",
        // IFC has no pivot door
        O::Pivot => ".USERDEFINED.,'OFFSET_PIVOT'",
        O::CentrePivot => ".USERDEFINED.,'CENTRE_PIVOT'",
        O::Roller => ".ROLLINGUP.,$",
        O::Revolving => ".REVOLVING.,$",
        // IFC4 has no sectional door
        O::Sectional => ".USERDEFINED.,'SECTIONAL_OVERHEAD'",
    }
}

/// A window's IFC4 partitioning type and its user-defined partitioning
/// (the last two attributes of an IfcWindow), from its grid.
fn window_partitioning(g: &creabim_core::cells::Grid) -> String {
    let (cols, rows) = (g.columns.len(), g.rows.len());
    match (cols, rows) {
        (1, 1) => ".SINGLE_PANEL.,$".into(),
        (2, 1) => ".DOUBLE_PANEL_VERTICAL.,$".into(),
        (1, 2) => ".DOUBLE_PANEL_HORIZONTAL.,$".into(),
        (3, 1) => ".TRIPLE_PANEL_VERTICAL.,$".into(),
        (1, 3) => ".TRIPLE_PANEL_HORIZONTAL.,$".into(),
        _ => format!(".USERDEFINED.,'GRID_{cols}X{rows}'"),
    }
}

/// A kept property, written.
fn prop_entity(c: &mut Ctx, q: &creabim_core::ifcdata::Prop) -> Ref {
    match q.kind.as_str() {
        "enumerated" => c.w.add(
            "IFCPROPERTYENUMERATEDVALUE",
            format!("{},$,{},$", string(&q.name), q.value),
        ),
        _ => c.w.add(
            "IFCPROPERTYSINGLEVALUE",
            format!("{},$,{},$", string(&q.name), q.value),
        ),
    }
}

/// The property sets and classification references kept from the file a
/// project was read from ([`creabim_core::ifcdata`]), written back on the
/// objects still in it. A set is written once and given to every object
/// that had one the same, as files share them. Where CreaBIM wrote a set
/// of the same name on an object already (a common set carrying the
/// phase's Status), its values stand (the model is the newer word) and the
/// file's other properties are added to it.
fn write_kept(c: &mut Ctx, model: &Model) {
    use creabim_core::ifcdata::{ClassRef, Pset};
    let data = model.ifc_data();
    if data.is_empty() {
        return;
    }
    let wanted: std::collections::HashSet<String> = model
        .iter()
        .map(|e| e.id.ifc_guid())
        .filter(|g| data.get(g).is_some())
        .map(|g| string(&g))
        .collect();
    let written = c.w.objects_with(&wanted);
    let own = c.w.sets_on();
    // each distinct set with the objects it goes on, in a steady order
    let mut sets: Vec<(&Pset, Vec<Ref>)> = Vec::new();
    let mut set_at: HashMap<String, usize> = HashMap::new();
    let mut classes: Vec<(&ClassRef, Vec<Ref>)> = Vec::new();
    for (guid, rec) in &data.by_guid {
        let Some(&obj) = written.get(&string(guid)) else {
            continue;
        };
        for p in &rec.psets {
            if let Some(&(set, n)) = own.get(&(obj.0, string(&p.name))) {
                // CreaBIM wrote a set of this name here (a common set with
                // the phase's Status): its values stand, and the file's
                // other properties join them, when the set is this
                // object's alone
                if n == 1 {
                    let have = c.w.prop_names(set);
                    let more: Vec<Ref> = p
                        .props
                        .iter()
                        .filter(|q| !have.contains(&string(&q.name)))
                        .map(|q| prop_entity(c, q))
                        .collect();
                    c.w.append_to_set(set, &more);
                }
                continue;
            }
            let key = serde_json::to_string(p).unwrap_or_default();
            let i = *set_at.entry(key).or_insert_with(|| {
                sets.push((p, Vec::new()));
                sets.len() - 1
            });
            sets[i].1.push(obj);
        }
        for k in &rec.classes {
            match classes.iter_mut().find(|(x, _)| *x == k) {
                Some((_, objs)) => objs.push(obj),
                None => classes.push((k, vec![obj])),
            }
        }
    }
    let opt = |v: &Option<String>| v.as_deref().map_or_else(|| "$".to_string(), string);
    for (p, objs) in sets {
        let props: Vec<Ref> = p.props.iter().map(|q| prop_entity(c, q)).collect();
        if props.is_empty() {
            continue;
        }
        let g = c.guid();
        let set = c.w.add(
            "IFCPROPERTYSET",
            format!(
                "{g},$,{},{},{}",
                string(&p.name),
                opt(&p.description),
                list(&props)
            ),
        );
        // a type holds its sets itself (HasPropertySets); an object
        // through a relationship
        let (types, objs): (Vec<Ref>, Vec<Ref>) = objs.into_iter().partition(|r| {
            let cl = c.w.class_of(*r);
            cl.ends_with("TYPE") || cl.ends_with("STYLE")
        });
        for t in types {
            c.w.add_to_list_arg(t, 5, &[set]);
        }
        if !objs.is_empty() {
            let g = c.guid();
            c.w.add(
                "IFCRELDEFINESBYPROPERTIES",
                format!("{g},$,$,$,{},{set}", list(&objs)),
            );
        }
    }
    // the systems once each, then each reference to its objects
    #[allow(clippy::type_complexity)]
    let mut systems: Vec<((Option<String>, Option<String>, Option<String>), Ref)> = Vec::new();
    for (k, objs) in classes {
        let sys = (k.system.clone(), k.edition.clone(), k.source.clone());
        let system = if sys == (None, None, None) {
            None
        } else if let Some((_, r)) = systems.iter().find(|(s, _)| *s == sys) {
            Some(*r)
        } else {
            // IfcClassification: Source, Edition, EditionDate, Name, ...
            let r = c.w.add(
                "IFCCLASSIFICATION",
                format!(
                    "{},{},$,{},$,$,$",
                    opt(&k.source),
                    opt(&k.edition),
                    string(k.system.as_deref().unwrap_or(""))
                ),
            );
            systems.push((sys, r));
            Some(r)
        };
        let relating = if k.code.is_none() && k.name.is_none() && k.location.is_none() {
            // a system given on its own
            match system {
                Some(r) => r,
                None => continue,
            }
        } else {
            // IfcClassificationReference: Location, Identification, Name, ReferencedSource, ...
            c.w.add(
                "IFCCLASSIFICATIONREFERENCE",
                format!(
                    "{},{},{},{},$,$",
                    opt(&k.location),
                    opt(&k.code),
                    opt(&k.name),
                    system.map_or_else(|| "$".to_string(), |r| r.to_string())
                ),
            )
        };
        let g = c.guid();
        c.w.add(
            "IFCRELASSOCIATESCLASSIFICATION",
            format!("{g},$,$,$,{},{relating}", list(&objs)),
        );
    }
}

/// The project's design options not made from a group of the file, as
/// groups to write: named by the option, described by its question, each
/// holding the pieces in that option alone, with a GlobalId that stays the
/// same from one export to the next.
fn option_groups(
    model: &Model,
    have: &[(creabim_core::groups::GroupDef, Vec<ElementId>)],
) -> Vec<(creabim_core::groups::GroupDef, Vec<ElementId>)> {
    use creabim_core::groups::{GroupDef, IfcGroupSource};
    let Some((project, _)) = model.project_info() else {
        return vec![];
    };
    let linked: std::collections::HashSet<u32> =
        have.iter().filter_map(|(g, _)| g.option).collect();
    let base = project.ifc_guid();
    let mut out = vec![];
    for q in creabim_core::phases::questions(model) {
        for o in q.options.iter().filter(|o| !linked.contains(&o.id)) {
            let g = GroupDef {
                id: 0,
                name: o.name.clone(),
                members: vec![],
                ifc: Some(IfcGroupSource {
                    guid: option_guid(&base, q.id, o.id),
                    description: Some(q.name.clone()),
                    object_type: Some("CreaBIM option".into()),
                }),
                option: Some(o.id),
            };
            let holds = creabim_core::groups::holds(model, &g);
            if !holds.is_empty() {
                out.push((g, holds));
            }
        }
    }
    out
}

/// A GlobalId for option `o` of question `q`: the project's, its last four
/// characters standing for the two (IFC's base 64 has 64 to a character).
fn option_guid(project: &str, q: u32, o: u32) -> String {
    const B64: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_$";
    let mut n = (q as u64 & 0xFFF) << 12 | (o as u64 & 0xFFF);
    let mut tail = [b'0'; 4];
    for t in tail.iter_mut().rev() {
        *t = B64[(n % 64) as usize];
        n /= 64;
    }
    format!(
        "{}{}",
        &project[..18],
        std::str::from_utf8(&tail).unwrap_or("0000")
    )
}

/// The base quantities of walls, slabs, roofs, columns, beams, doors and
/// windows (Qto_WallBaseQuantities...), from the take-off: the numbers the
/// Data (IFC) tab shows.
fn write_quantities(c: &mut Ctx, model: &Model) {
    #[derive(Default)]
    struct Q {
        length: f64,
        area: f64,
        volume: f64,
    }
    let mut by: HashMap<ElementId, Q> = HashMap::new();
    for l in creabim_core::takeoff::lines(model) {
        let q = by.entry(l.element).or_default();
        q.length = q.length.max(l.quantity.length);
        q.area = q.area.max(l.quantity.area);
        q.volume += l.quantity.volume;
    }
    let wanted: std::collections::HashSet<String> =
        model.iter().map(|e| string(&e.id.ifc_guid())).collect();
    let written = c.w.objects_with(&wanted);
    let own = c.w.sets_on();
    // (set name, [(quantity name, kind, value)]) a kind of element
    for e in model.iter() {
        let Some(&r) = written.get(&string(&e.id.ifc_guid())) else {
            continue;
        };
        let q = by.remove(&e.id).unwrap_or_default();
        let (set, rows): (&str, Vec<(&str, char, f64)>) = match &e.kind {
            ElementKind::Wall(w) => (
                "Qto_WallBaseQuantities",
                vec![
                    ("Length", 'l', q.length),
                    ("Height", 'l', w.height),
                    ("Width", 'l', w.thickness),
                    ("NetSideArea", 'a', q.area),
                    ("NetVolume", 'v', q.volume),
                ],
            ),
            ElementKind::Slab(sl) => (
                "Qto_SlabBaseQuantities",
                vec![
                    ("Depth", 'l', sl.thickness),
                    ("NetArea", 'a', q.area),
                    ("NetVolume", 'v', q.volume),
                ],
            ),
            ElementKind::Roof(_) => (
                "Qto_RoofBaseQuantities",
                vec![("GrossArea", 'a', q.area), ("NetVolume", 'v', q.volume)],
            ),
            ElementKind::Column(_) => (
                "Qto_ColumnBaseQuantities",
                vec![("Length", 'l', q.length), ("NetVolume", 'v', q.volume)],
            ),
            ElementKind::Beam(_) => (
                "Qto_BeamBaseQuantities",
                vec![("Length", 'l', q.length), ("NetVolume", 'v', q.volume)],
            ),
            ElementKind::Opening(o) if o.kind == creabim_core::OpeningKind::Door => (
                "Qto_DoorBaseQuantities",
                vec![
                    ("Width", 'l', o.width),
                    ("Height", 'l', o.height),
                    ("Area", 'a', o.width * o.height),
                ],
            ),
            ElementKind::Opening(o) if o.kind == creabim_core::OpeningKind::Window => (
                "Qto_WindowBaseQuantities",
                vec![
                    ("Width", 'l', o.width),
                    ("Height", 'l', o.height),
                    ("Area", 'a', o.width * o.height),
                ],
            ),
            _ => continue,
        };
        if own.contains_key(&(r.0, string(set))) {
            continue;
        }
        let refs: Vec<Ref> = rows
            .iter()
            .filter(|(_, _, v)| v.is_finite() && *v > 0.0)
            .map(|(n, k, v)| {
                let class = match k {
                    'l' => "IFCQUANTITYLENGTH",
                    'a' => "IFCQUANTITYAREA",
                    _ => "IFCQUANTITYVOLUME",
                };
                c.w.add(class, format!("{},$,$,{},$", string(n), real(*v)))
            })
            .collect();
        if refs.is_empty() {
            continue;
        }
        let g = c.guid();
        let qs = c.w.add(
            "IFCELEMENTQUANTITY",
            format!("{g},$,{},$,$,{}", string(set), list(&refs)),
        );
        let g = c.guid();
        c.w.add("IFCRELDEFINESBYPROPERTIES", format!("{g},$,$,$,({r}),{qs}"));
    }
}

/// The common sets (Pset_WallCommon, Pset_DoorCommon...) of walls, slabs,
/// roofs, columns, beams, doors and windows: what is declared, load-bearing
/// and external as [`creabim_core::common`] works them out. What is not
/// set or not known is left out, never guessed. Where CreaBIM wrote the
/// set already (a phased element's Status), these join it.
/// Each document given to an element or type in CreaBIM: an
/// IfcDocumentReference (Location, Identification, Name, Description,
/// ReferencedDocument) on an IfcRelAssociatesDocument with everything that
/// has it, as Bonsai writes them.
fn write_documents(c: &mut Ctx, model: &Model) {
    let wanted: std::collections::HashSet<String> = model
        .iter()
        .filter(|e| !e.documents.is_empty())
        .map(|e| string(&e.id.ifc_guid()))
        .collect();
    if wanted.is_empty() {
        return;
    }
    let written = c.w.objects_with(&wanted);
    let mut holders: Vec<(&creabim_core::docrefs::DocRef, Vec<Ref>)> = Vec::new();
    for e in model.iter() {
        let Some(&r) = written.get(&string(&e.id.ifc_guid())) else {
            continue;
        };
        for d in &e.documents {
            match holders.iter_mut().find(|(x, _)| *x == d) {
                Some((_, rs)) => rs.push(r),
                None => holders.push((d, vec![r])),
            }
        }
    }
    for (d, rs) in holders {
        let name = if d.name.trim().is_empty() {
            "$".to_string()
        } else {
            string(&d.name)
        };
        let doc = c.w.add(
            "IFCDOCUMENTREFERENCE",
            format!("{},$,{name},$,$", string(&d.location)),
        );
        let g = c.guid();
        c.w.add(
            "IFCRELASSOCIATESDOCUMENT",
            format!("{g},$,$,$,{},{doc}", list(&rs)),
        );
    }
}

fn write_common(c: &mut Ctx, model: &Model) {
    use crate::params::Prop;
    let wanted: std::collections::HashSet<String> = model
        .iter()
        .filter(|e| creabim_core::common::rated(&e.kind))
        .map(|e| string(&e.id.ifc_guid()))
        .collect();
    if wanted.is_empty() {
        return;
    }
    let written = c.w.objects_with(&wanted);
    let own = c.w.sets_on();
    for e in model.iter() {
        let set = match &e.kind {
            ElementKind::Wall(_) => "Pset_WallCommon",
            ElementKind::Slab(_) => "Pset_SlabCommon",
            ElementKind::Roof(_) => "Pset_RoofCommon",
            ElementKind::Column(_) => "Pset_ColumnCommon",
            ElementKind::Beam(_) => "Pset_BeamCommon",
            ElementKind::Opening(o) if o.kind == OpeningKind::Door => "Pset_DoorCommon",
            ElementKind::Opening(o) if o.kind == OpeningKind::Window => "Pset_WindowCommon",
            _ => continue,
        };
        let Some(&r) = written.get(&string(&e.id.ifc_guid())) else {
            continue;
        };
        let cm = creabim_core::common::common(model, e);
        let d = &cm.declared;
        let mut props: Vec<(&'static str, Prop)> = vec![];
        if let Some((v, _)) = cm.external {
            props.push(("IsExternal", Prop::Bool(v)));
        }
        if let Some((v, _)) = cm.load_bearing {
            props.push(("LoadBearing", Prop::Bool(v)));
        }
        for (k, v) in [
            ("FireRating", &d.fire),
            ("AcousticRating", &d.acoustic),
            ("SecurityRating", &d.security),
        ] {
            if !v.trim().is_empty() {
                props.push((k, Prop::Label(v.clone())));
            }
        }
        if let Some(u) = d.u_value {
            props.push(("ThermalTransmittance", Prop::Thermal(u)));
        }
        if d.accessible {
            props.push(("HandicapAccessible", Prop::Bool(true)));
        }
        if props.is_empty() {
            continue;
        }
        match own.get(&(r.0, string(set))) {
            // CreaBIM's own set on it already, its alone: these join it
            Some(&(existing, 1)) => {
                let have = c.w.prop_names(existing);
                let more: Vec<Ref> = props
                    .iter()
                    .filter(|(k, _)| !have.contains(&string(k)))
                    .map(|(k, v)| {
                        c.w.add(
                            "IFCPROPERTYSINGLEVALUE",
                            format!("{},$,{},$", string(k), v.step()),
                        )
                    })
                    .collect();
                c.w.append_to_set(existing, &more);
            }
            _ => c.pset_on(&[r], set, &props),
        }
    }
}

#[cfg(test)]
mod project_tests {
    use super::*;

    #[test]
    fn options_take_the_projects_names() {
        let mut doc = creabim_core::Document::new();
        let o = ExportOptions::for_model(doc.model(), "File name");
        assert_eq!(o.project_name, "File name");
        let info = creabim_core::ProjectInfo {
            name: "Maple Lane House".into(),
            site_name: "Plot 4".into(),
            building_name: "House".into(),
            ..Default::default()
        };
        doc.commit(creabim_core::set_project(doc.model(), info))
            .unwrap();
        let o = ExportOptions::for_model(doc.model(), "File name");
        assert_eq!(o.project_name, "Maple Lane House");
        assert_eq!(o.site_name, "Plot 4");
        assert_eq!(o.building_name, "House");
    }

    #[test]
    fn what_is_part_of_the_site_is_placed_in_the_site() {
        let mut model = creabim_core::starter::starter_house();
        let stair = model
            .iter()
            .find(|e| matches!(e.kind, ElementKind::Stair(_)))
            .unwrap()
            .clone();
        // which relation holds the stair, by the entity it is contained in
        let holder = |ifc: &str| -> String {
            let guid = stair.id.ifc_guid();
            let r = ifc
                .lines()
                .find(|l| l.contains(&guid) && l.contains("=IFCSTAIR"))
                .and_then(|l| l.split('=').next())
                .expect("the stair is exported")
                .to_string();
            let rel = ifc
                .lines()
                .find(|l| {
                    l.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE")
                        && (l.contains(&format!("{r},")) || l.contains(&format!("{r})")))
                })
                .expect("the stair is contained somewhere");
            // the last argument is what it is contained in
            let within = rel
                .trim_end_matches(");")
                .rsplit(',')
                .next()
                .unwrap()
                .to_string();
            ifc.lines()
                .find(|l| l.starts_with(&format!("{within}=")))
                .unwrap()
                .split('(')
                .next()
                .unwrap()
                .to_string()
        };
        let o = ExportOptions::for_model(&model, "Starter");
        assert!(holder(&export_ifc4(&model, &o)).ends_with("IFCBUILDINGSTOREY"));
        let mut outdoors = stair.clone();
        outdoors.on_site = true;
        creabim_core::Transaction::new("t")
            .replace(stair.clone(), outdoors)
            .apply(&mut model)
            .unwrap();
        assert!(holder(&export_ifc4(&model, &o)).ends_with("IFCSITE"));
    }

    #[test]
    fn grid_lines_go_out_as_an_ifc_grid_with_their_labels() {
        let model = creabim_core::starter::starter_house();
        let lines = model
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::Grid(_)))
            .count();
        assert!(lines > 0);
        let ifc = export_ifc4(&model, &ExportOptions::for_model(&model, "Starter"));
        assert!(ifc.contains("IFCGRID("), "no IfcGrid");
        assert_eq!(ifc.matches("IFCGRIDAXIS(").count(), lines);
        assert!(ifc.contains("IFCGRIDAXIS('A',"), "an axis keeps its label");
        // contained in the spatial structure
        let grid_ref = ifc
            .lines()
            .find(|l| l.contains("=IFCGRID("))
            .and_then(|l| l.split('=').next())
            .unwrap()
            .to_string();
        assert!(ifc
            .lines()
            .any(|l| l.contains("IFCRELCONTAINEDINSPATIALSTRUCTURE") && l.contains(&grid_ref)));
    }
}

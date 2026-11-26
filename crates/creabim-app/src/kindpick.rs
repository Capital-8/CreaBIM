//! Choosing a door or window: every way a door or window opens, as a
//! picture, in one picker reached from the tool before placing, from
//! Properties after, and from Create. The pictures are drawn here, as SVG
//! for the picker's page.

use creabim_core::cells::{Assembly, WindowStyle};
use creabim_core::{Model, Opening, OpeningFrame, OpeningKind, Operation};

/// A kind a new or selected door or window is made as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Door(Operation),
    Window(WindowStyle),
}

/// The families the picker groups kinds in, in order.
pub const DOORS: &[(&str, &[Operation])] = &[
    ("Hinged", &[Operation::Standard, Operation::DoubleSwing]),
    (
        "Sliding",
        &[
            Operation::SurfaceSliding,
            Operation::SurfaceSlidingPair,
            Operation::Pocket,
            Operation::PocketPair,
            Operation::MultiSlide,
        ],
    ),
    (
        "Pivot & two-way",
        &[
            Operation::Pivot,
            Operation::CentrePivot,
            Operation::DoubleAction,
            Operation::DoubleActionPair,
        ],
    ),
    (
        "Folding",
        &[Operation::Bifold, Operation::BifoldPair, Operation::Folding],
    ),
    ("Lift & roll", &[Operation::Sectional, Operation::Roller]),
    ("Revolving", &[Operation::Revolving]),
];

pub const WINDOWS: &[(&str, &[WindowStyle])] = &[
    ("Fixed", &[WindowStyle::Fixed]),
    (
        "Hinged",
        &[
            WindowStyle::Casement,
            WindowStyle::CasementPair,
            WindowStyle::Awning,
            WindowStyle::Hopper,
            WindowStyle::TiltTurn,
        ],
    ),
    (
        "Sliding & hung",
        &[
            WindowStyle::Sliding,
            WindowStyle::SingleHung,
            WindowStyle::DoubleHung,
            WindowStyle::Guillotine,
        ],
    ),
    (
        "Pivot",
        &[WindowStyle::HorizontalPivot, WindowStyle::VerticalPivot],
    ),
    (
        "Folding & louvred",
        &[WindowStyle::Bifold, WindowStyle::Louvred],
    ),
];

impl Pick {
    pub fn label(self) -> &'static str {
        match self {
            Pick::Door(op) => op.label(OpeningKind::Door),
            Pick::Window(s) => s.label(),
        }
    }

    pub fn family(self) -> &'static str {
        match self {
            Pick::Door(op) => DOORS.iter().find(|f| f.1.contains(&op)).map_or("", |f| f.0),
            Pick::Window(s) => WINDOWS
                .iter()
                .find(|f| f.1.contains(&s))
                .map_or("", |f| f.0),
        }
    }

    /// Its id in an action: "d:Pivot", "w:Casement".
    pub fn id(self) -> String {
        match self {
            Pick::Door(op) => format!("d:{op:?}"),
            Pick::Window(s) => format!("w:{s:?}"),
        }
    }

    pub fn parse(id: &str) -> Option<Pick> {
        if let Some(n) = id.strip_prefix("d:") {
            return DOORS
                .iter()
                .flat_map(|f| f.1.iter())
                .find(|op| format!("{op:?}") == n)
                .map(|op| Pick::Door(*op));
        }
        let n = id.strip_prefix("w:")?;
        WINDOWS
            .iter()
            .flat_map(|f| f.1.iter())
            .find(|s| format!("{s:?}") == n)
            .map(|s| Pick::Window(*s))
    }

    /// The kind a door or window is, if it is one the picker offers.
    pub fn of(o: &Opening) -> Option<Pick> {
        match o.kind {
            OpeningKind::Door => Some(Pick::Door(o.frame.operation(OpeningKind::Door))),
            OpeningKind::Window => WindowStyle::of(&o.frame.core()).map(Pick::Window),
            OpeningKind::Hole => None,
        }
    }

    pub fn of_frame(kind: OpeningKind, f: &OpeningFrame) -> Option<Pick> {
        match kind {
            OpeningKind::Door => Some(Pick::Door(f.operation(OpeningKind::Door))),
            OpeningKind::Window => WindowStyle::of(&f.core()).map(Pick::Window),
            OpeningKind::Hole => None,
        }
    }

    pub fn kind(self) -> OpeningKind {
        match self {
            Pick::Door(_) => OpeningKind::Door,
            Pick::Window(_) => OpeningKind::Window,
        }
    }

    /// A new frame of this kind.
    pub fn frame(self, model: &Model) -> OpeningFrame {
        let mut f = OpeningFrame::for_kind(self.kind());
        self.apply_frame(model, &mut f);
        f
    }

    /// Make a frame this kind, keeping its lights, head and sizes.
    pub fn apply_frame(self, model: &Model, f: &mut OpeningFrame) {
        match self {
            Pick::Door(op) => f.set_operation(OpeningKind::Door, op),
            Pick::Window(s) => {
                let inward = model.project_info().is_some_and(|(_, p)| {
                    p.openings.casements == creabim_core::CasementSide::Inward
                });
                let g = s.grid(inward, f.panes());
                f.grid = match f.assembly() {
                    Some(a) => Assembly { core: g, ..a }.grid(),
                    None => creabim_core::cells::Grid {
                        outline: f.grid.outline,
                        ..g
                    },
                };
            }
        }
    }

    pub fn glyph(self) -> Vec<G> {
        match self {
            Pick::Door(op) => door(op),
            Pick::Window(s) => window(s),
        }
    }

    /// The picture's frame: doors are drawn in plan, windows in elevation.
    pub fn view_box(self) -> (f32, f32) {
        match self {
            Pick::Door(_) => (40.0, 40.0),
            Pick::Window(_) => (40.0, 42.0),
        }
    }

    pub fn svg(self) -> String {
        svg(&self.glyph(), self.view_box())
    }
}

/// How a stroke is drawn: the leaf (heavy), its swing (thin) or what is
/// out of sight (dashed).
#[derive(Clone, Copy)]
pub enum Pen {
    Heavy,
    Thin,
    Dash,
}

/// One mark of a picture, in its own 40-unit frame, y down.
pub enum G {
    Wall(f32, f32, f32, f32),
    Line(Vec<(f32, f32)>, Pen),
    /// Centre, radius, from and to in degrees (y down, clockwise).
    Arc((f32, f32), f32, f32, f32, Pen),
    Circle((f32, f32), f32, Pen),
}

fn l(p: &[(f32, f32)]) -> G {
    G::Line(p.to_vec(), Pen::Heavy)
}
fn s(p: &[(f32, f32)]) -> G {
    G::Line(p.to_vec(), Pen::Thin)
}
fn d(p: &[(f32, f32)]) -> G {
    G::Line(p.to_vec(), Pen::Dash)
}
/// An arrow from `a` to `b` along a level line.
fn arrow(a: f32, b: f32, y: f32) -> Vec<G> {
    let k = if b > a { -3.0 } else { 3.0 };
    vec![
        s(&[(a, y), (b, y)]),
        s(&[(b + k, y - 2.0), (b, y), (b + k, y + 2.0)]),
    ]
}
fn jambs(w: f32) -> Vec<G> {
    vec![G::Wall(0.0, 25.0, w, 6.0), G::Wall(40.0 - w, 25.0, w, 6.0)]
}

fn door(op: Operation) -> Vec<G> {
    use Operation as O;
    let mut g = vec![];
    match op {
        O::Standard => {
            g.extend(jambs(7.0));
            g.push(l(&[(8.0, 25.0), (8.0, 5.0)]));
            g.push(G::Arc((8.0, 25.0), 20.0, -90.0, 0.0, Pen::Thin));
        }
        O::DoubleSwing => {
            g.extend(jambs(5.0));
            g.push(l(&[(6.0, 25.0), (6.0, 12.0)]));
            g.push(l(&[(34.0, 25.0), (34.0, 12.0)]));
            g.push(G::Arc((6.0, 25.0), 13.0, -90.0, 0.0, Pen::Thin));
            g.push(G::Arc((34.0, 25.0), 13.0, 180.0, 270.0, Pen::Thin));
        }
        O::SurfaceSliding => {
            g.extend(jambs(7.0));
            g.push(l(&[(20.0, 23.0), (36.0, 23.0)]));
            g.extend(arrow(24.0, 12.0, 18.0));
        }
        O::SurfaceSlidingPair => {
            g.extend(jambs(5.0));
            g.push(l(&[(6.0, 23.0), (19.0, 23.0)]));
            g.push(l(&[(21.0, 23.0), (34.0, 23.0)]));
            g.extend(arrow(14.0, 6.0, 18.0));
            g.extend(arrow(26.0, 34.0, 18.0));
        }
        O::Pocket => {
            g.push(G::Wall(0.0, 25.0, 22.0, 6.0));
            g.push(G::Wall(35.0, 25.0, 5.0, 6.0));
            g.push(d(&[(4.0, 28.0), (20.0, 28.0)]));
            g.push(l(&[(20.0, 28.0), (34.0, 28.0)]));
            g.extend(arrow(30.0, 18.0, 20.0));
        }
        O::PocketPair => {
            g.push(G::Wall(0.0, 25.0, 13.0, 6.0));
            g.push(G::Wall(27.0, 25.0, 13.0, 6.0));
            g.push(d(&[(2.0, 28.0), (12.0, 28.0)]));
            g.push(d(&[(28.0, 28.0), (38.0, 28.0)]));
            g.push(l(&[(12.0, 28.0), (19.0, 28.0)]));
            g.push(l(&[(21.0, 28.0), (28.0, 28.0)]));
            g.extend(arrow(17.0, 11.0, 20.0));
            g.extend(arrow(23.0, 29.0, 20.0));
        }
        O::MultiSlide => {
            g.extend(jambs(3.0));
            g.push(l(&[(4.0, 24.0), (13.0, 24.0)]));
            g.push(l(&[(12.0, 27.0), (22.0, 27.0)]));
            g.push(l(&[(21.0, 24.0), (30.0, 24.0)]));
            g.push(l(&[(29.0, 27.0), (36.0, 27.0)]));
            g.extend(arrow(24.0, 16.0, 18.0));
        }
        O::Pivot => {
            g.extend(jambs(5.0));
            g.push(l(&[(12.0, 33.0), (12.0, 8.0)]));
            g.push(G::Arc((12.0, 25.0), 17.0, -90.0, 0.0, Pen::Thin));
            g.push(G::Arc((12.0, 25.0), 7.0, 90.0, 180.0, Pen::Thin));
            g.push(s(&[(10.0, 25.0), (14.0, 25.0)]));
            g.push(s(&[(12.0, 23.0), (12.0, 27.0)]));
        }
        O::CentrePivot => {
            g.extend(jambs(5.0));
            g.push(l(&[(20.0, 39.0), (20.0, 12.0)]));
            g.push(G::Arc((20.0, 26.0), 14.0, -90.0, 0.0, Pen::Thin));
            g.push(G::Arc((20.0, 26.0), 13.0, 90.0, 180.0, Pen::Thin));
        }
        O::DoubleAction => {
            g.extend(jambs(7.0));
            g.push(l(&[(8.0, 28.0), (8.0, 8.0)]));
            g.push(G::Arc((8.0, 28.0), 20.0, -90.0, 0.0, Pen::Thin));
            g.push(G::Arc((8.0, 28.0), 20.0, 0.0, 70.0, Pen::Dash));
        }
        O::DoubleActionPair => {
            g.extend(jambs(5.0));
            g.push(l(&[(6.0, 28.0), (6.0, 14.0)]));
            g.push(l(&[(34.0, 28.0), (34.0, 14.0)]));
            g.push(G::Arc((6.0, 28.0), 14.0, -90.0, 0.0, Pen::Thin));
            g.push(G::Arc((34.0, 28.0), 14.0, 180.0, 270.0, Pen::Thin));
            g.push(G::Arc((6.0, 28.0), 14.0, 0.0, 60.0, Pen::Dash));
            g.push(G::Arc((34.0, 28.0), 14.0, 120.0, 180.0, Pen::Dash));
        }
        O::Bifold => {
            g.extend(jambs(7.0));
            g.push(l(&[(8.0, 25.0), (14.0, 12.0), (20.0, 25.0)]));
            g.extend(arrow(30.0, 22.0, 21.0));
        }
        O::BifoldPair => {
            g.extend(jambs(5.0));
            g.push(l(&[(6.0, 25.0), (10.0, 14.0), (14.0, 25.0)]));
            g.push(l(&[(34.0, 25.0), (30.0, 14.0), (26.0, 25.0)]));
        }
        O::Folding => {
            g.extend(jambs(3.0));
            g.push(l(&[
                (4.0, 25.0),
                (8.0, 15.0),
                (12.0, 25.0),
                (16.0, 15.0),
                (20.0, 25.0),
            ]));
            g.extend(arrow(34.0, 24.0, 21.0));
        }
        O::Sectional => {
            g.extend(jambs(4.0));
            g.push(d(&[
                (5.0, 10.0),
                (35.0, 10.0),
                (35.0, 25.0),
                (5.0, 25.0),
                (5.0, 10.0),
            ]));
            g.push(l(&[(5.0, 28.0), (35.0, 28.0)]));
        }
        O::Roller => {
            g.extend(jambs(4.0));
            g.push(l(&[(5.0, 28.0), (35.0, 28.0)]));
            g.push(G::Circle((20.0, 18.0), 5.0, Pen::Thin));
        }
        O::Revolving => {
            g.push(G::Circle((20.0, 22.0), 14.0, Pen::Thin));
            g.push(l(&[(6.0, 22.0), (34.0, 22.0)]));
            g.push(l(&[(20.0, 8.0), (20.0, 36.0)]));
        }
    }
    g
}

fn window(st: WindowStyle) -> Vec<G> {
    use WindowStyle as W;
    let frame = |x: f32, y: f32, w: f32, h: f32| {
        l(&[(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)])
    };
    let mut g = vec![];
    match st {
        W::Fixed => g.push(frame(6.0, 4.0, 28.0, 34.0)),
        W::Casement => {
            g.push(frame(6.0, 4.0, 28.0, 34.0));
            g.push(s(&[(6.0, 4.0), (34.0, 21.0), (6.0, 38.0)]));
        }
        W::CasementPair => {
            g.push(frame(4.0, 4.0, 32.0, 34.0));
            g.push(l(&[(20.0, 4.0), (20.0, 38.0)]));
            g.push(s(&[(4.0, 4.0), (20.0, 21.0), (4.0, 38.0)]));
            g.push(s(&[(36.0, 4.0), (20.0, 21.0), (36.0, 38.0)]));
        }
        W::Awning => {
            g.push(frame(6.0, 8.0, 28.0, 26.0));
            g.push(s(&[(6.0, 34.0), (20.0, 8.0), (34.0, 34.0)]));
        }
        W::Hopper => {
            g.push(frame(6.0, 8.0, 28.0, 26.0));
            g.push(d(&[(6.0, 8.0), (20.0, 34.0), (34.0, 8.0)]));
        }
        W::Sliding => {
            g.push(frame(4.0, 8.0, 32.0, 26.0));
            g.push(l(&[(20.0, 8.0), (20.0, 34.0)]));
            g.extend(arrow(26.0, 32.0, 21.0));
        }
        W::Louvred => {
            g.push(frame(6.0, 4.0, 28.0, 34.0));
            for y in [10.0, 16.0, 22.0, 28.0, 34.0] {
                g.push(s(&[(6.0, y), (34.0, y)]));
            }
        }
        W::TiltTurn => {
            g.push(frame(6.0, 4.0, 28.0, 34.0));
            g.push(s(&[(6.0, 4.0), (34.0, 21.0), (6.0, 38.0)]));
            g.push(d(&[(6.0, 38.0), (20.0, 4.0), (34.0, 38.0)]));
        }
        W::SingleHung | W::DoubleHung | W::Guillotine => {
            g.push(frame(8.0, 2.0, 24.0, 38.0));
            g.push(l(&[(8.0, 21.0), (32.0, 21.0)]));
            let up = |y0: f32, y1: f32| {
                vec![
                    s(&[(20.0, y0), (20.0, y1)]),
                    s(&[(18.0, y1 + 2.0), (20.0, y1), (22.0, y1 + 2.0)]),
                ]
            };
            let down = |y0: f32, y1: f32| {
                vec![
                    s(&[(20.0, y0), (20.0, y1)]),
                    s(&[(18.0, y1 - 2.0), (20.0, y1), (22.0, y1 - 2.0)]),
                ]
            };
            match st {
                W::SingleHung => g.extend(up(34.0, 26.0)),
                W::DoubleHung => {
                    g.extend(up(34.0, 26.0));
                    g.extend(down(8.0, 16.0));
                }
                _ => g.extend(up(30.0, 12.0)),
            }
        }
        W::HorizontalPivot => {
            g.push(frame(6.0, 4.0, 28.0, 34.0));
            g.push(s(&[(6.0, 21.0), (34.0, 21.0)]));
            g.push(s(&[(6.0, 4.0), (20.0, 21.0), (34.0, 4.0)]));
            g.push(d(&[(6.0, 38.0), (20.0, 21.0), (34.0, 38.0)]));
        }
        W::VerticalPivot => {
            g.push(frame(6.0, 4.0, 28.0, 34.0));
            g.push(s(&[(20.0, 4.0), (20.0, 38.0)]));
            g.push(s(&[(6.0, 4.0), (20.0, 21.0), (6.0, 38.0)]));
            g.push(d(&[(34.0, 4.0), (20.0, 21.0), (34.0, 38.0)]));
        }
        W::Bifold => {
            g.push(frame(4.0, 6.0, 32.0, 30.0));
            for x in [12.0, 20.0, 28.0] {
                g.push(l(&[(x, 6.0), (x, 36.0)]));
            }
        }
    }
    g
}

fn arc_points(c: (f32, f32), r: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
    let n = (((a1 - a0).abs() / 6.0).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
            (c.0 + r * a.cos(), c.1 + r * a.sin())
        })
        .collect()
}

/// The picture as SVG markup; the page gives the pens their colours
/// (`.kg-w`, `.kg-s`, `.kg-d`, `.kg-wall`).
pub fn svg(g: &[G], (w, h): (f32, f32)) -> String {
    let cls = |p: Pen| match p {
        Pen::Heavy => "kg-w",
        Pen::Thin => "kg-s",
        Pen::Dash => "kg-d",
    };
    let pts = |p: &[(f32, f32)]| {
        p.iter()
            .map(|(x, y)| format!("{x:.1},{y:.1}"))
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut out = format!("<svg viewBox='0 0 {w} {h}' class='kg'>");
    for m in g {
        out += &match m {
            G::Wall(x, y, ww, hh) => {
                format!("<rect class='kg-wall' x='{x}' y='{y}' width='{ww}' height='{hh}'/>")
            }
            G::Line(p, pen) => format!("<polyline class='{}' points='{}'/>", cls(*pen), pts(p)),
            G::Arc(c, r, a0, a1, pen) => format!(
                "<polyline class='{}' points='{}'/>",
                cls(*pen),
                pts(&arc_points(*c, *r, *a0, *a1))
            ),
            G::Circle(c, r, pen) => format!(
                "<circle class='{}' cx='{}' cy='{}' r='{r}'/>",
                cls(*pen),
                c.0,
                c.1
            ),
        };
    }
    out + "</svg>"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_door_and_window_kind_is_in_the_picker_once_and_comes_back_from_its_id() {
        let doors: Vec<Operation> = DOORS.iter().flat_map(|f| f.1.iter().copied()).collect();
        for op in Operation::for_kind(OpeningKind::Door) {
            assert_eq!(doors.iter().filter(|o| *o == op).count(), 1, "{op:?}");
        }
        let windows: Vec<WindowStyle> = WINDOWS.iter().flat_map(|f| f.1.iter().copied()).collect();
        for s in WindowStyle::ALL {
            assert_eq!(windows.iter().filter(|w| **w == s).count(), 1, "{s:?}");
        }
        for p in doors
            .iter()
            .map(|o| Pick::Door(*o))
            .chain(windows.iter().map(|s| Pick::Window(*s)))
        {
            assert_eq!(Pick::parse(&p.id()), Some(p));
            assert!(!p.glyph().is_empty());
        }
    }

    #[test]
    fn a_picked_kind_makes_a_frame_that_reads_back_as_that_kind() {
        let model = Model::new();
        for f in DOORS {
            for op in f.1 {
                let p = Pick::Door(*op);
                assert_eq!(Pick::of_frame(OpeningKind::Door, &p.frame(&model)), Some(p));
            }
        }
        for f in WINDOWS {
            for s in f.1 {
                let p = Pick::Window(*s);
                assert_eq!(
                    Pick::of_frame(OpeningKind::Window, &p.frame(&model)),
                    Some(p)
                );
            }
        }
    }
}

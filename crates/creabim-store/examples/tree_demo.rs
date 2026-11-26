use creabim_core::tree::{
    seed_at, Branching, ClumpForm, LeafHabit, TreeElevation, TreePreset, TreeShape,
};
use creabim_core::{Document, Element, ElementKind, Site, Transaction, View};
use glam::DVec2;
fn main() {
    let m = creabim_core::garden::garden_house();
    let g = m.iter().find(|e| e.name == "Ground Floor").unwrap().id;
    let (lo, _hi) = m
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::Wall(w) if w.level == g => Some((w.start, w.end)),
            _ => None,
        })
        .fold(
            (DVec2::splat(1e9), DVec2::splat(-1e9)),
            |(lo, hi), (a, b)| (lo.min(a).min(b), hi.max(a).max(b)),
        );
    let mut doc = Document::from_model(m);
    let mut tx = Transaction::new("demo trees");
    let x0 = lo.x - 14.0;
    let y0 = lo.y - 10.0;
    let put = |tx: Transaction,
               name: &str,
               at: DVec2,
               f: &dyn Fn(TreeShape) -> TreeShape,
               p: TreePreset| {
        let (s, c, h) = p.shape();
        let s = f(TreeShape {
            seed: seed_at(at),
            ..s
        });
        tx.insert(Element::new(
            name,
            ElementKind::Site(Site::tree(g, at, s, c, h)),
        ))
    };
    for (i, p) in TreePreset::ALL.iter().enumerate() {
        let at = DVec2::new(x0 - 12.0 * i as f64, y0);
        let e = TreeElevation::ALL[(i + 1) % TreeElevation::ALL.len()];
        tx = put(
            tx,
            p.label(),
            at,
            &move |s| TreeShape {
                elevation: e,
                leaves: LeafHabit::Deciduous,
                ..s
            },
            *p,
        );
        let at = DVec2::new(x0 - 12.0 * i as f64, y0 - 14.0);
        tx = put(
            tx,
            &format!("Bare {}", p.label()),
            at,
            &|s| TreeShape {
                leaves: LeafHabit::Deciduous,
                month: 1,
                own_month: true,
                ..s
            },
            *p,
        );
    }
    for (i, b) in Branching::ALL.iter().enumerate() {
        let at = DVec2::new(x0 - 12.0 * i as f64, y0 - 28.0);
        let b = *b;
        tx = put(
            tx,
            &format!("Bare {}", b.label()),
            at,
            &move |s| TreeShape {
                leaves: LeafHabit::Deciduous,
                month: 1,
                own_month: true,
                branching: b,
                ..s
            },
            TreePreset::Shade,
        );
    }
    for f in 1..=5u8 {
        let at = DVec2::new(x0 - 12.0 * (f - 1) as f64, y0 - 42.0);
        tx = put(
            tx,
            &format!("Low poly {f}"),
            at,
            &move |s| TreeShape {
                form: ClumpForm::LowPoly,
                facets: f,
                ..s
            },
            TreePreset::Shade,
        );
    }
    let x1 = x0 - 12.0 * 5.0;
    let mut v = View::section(DVec2::new(x1, y0), DVec2::new(x0 + 6.0, y0), 8.0, true);
    v.scale = 100.0;
    tx = tx.insert(Element::new("Section Cut", ElementKind::View(v)));
    let mut v = View::section(
        DVec2::new(x1, y0 - 22.0),
        DVec2::new(x0 + 6.0, y0 - 22.0),
        10.0,
        false,
    );
    v.scale = 100.0;
    tx = tx.insert(Element::new("Elevation Bare", ElementKind::View(v)));
    doc.commit(tx).unwrap();
    let out = std::env::args().nth(1).unwrap();
    creabim_store::save(&out, &doc).unwrap();
    println!("saved {out}");
}

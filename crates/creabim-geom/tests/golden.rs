//! Golden drawings: every view of the sample models is drawn and its
//! fingerprint compared with the one kept in `tests/golden/<model>.txt`.
//! A change that moves, adds or drops a line in any plan, section,
//! elevation, detail, schedule or 3D drawing fails here, so an
//! optimisation can't quietly change what is drawn.
//!
//! A fingerprint is one line a view: a hash of its SVG, and how many of
//! each kind of mark it has (the whole drawings would be 11 MB). Every
//! run also writes the drawings themselves to `target/golden-actual/`.
//! To see what a change did: run the test before it and keep that folder,
//! run it after, and compare the two SVG files of the view named in the
//! failure.
//!
//! When a drawing is meant to change, look at the difference, then keep
//! the new fingerprints:
//!
//! ```text
//! CREABIM_BLESS=1 cargo test -p creabim-geom --test golden
//! ```
//!
//! Coordinates are compared to a tenth of a paper millimetre: closer than
//! anything that prints, and clear of floating-point noise.
//!
//! The hash is of the marks, not of their order: sections, elevations and
//! 3D drawings list their lines in a different order on each run, and may
//! start a line from its other end (see "Run-to-run differences" in
//! `docs/PERFORMANCE.md`). So each mark's points are put in one canonical
//! order and the marks are sorted before hashing.

use creabim_core::{ElementKind, Model};
use std::path::PathBuf;

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn actual_dir() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
    target.join("golden-actual")
}

/// `svg` with every decimal number rounded to one place, so the comparison
/// is about the drawing and not about the last bits of a float. Whole
/// numbers, colours and names are left as they are.
fn rounded(svg: &str) -> String {
    let b = svg.as_bytes();
    let mut out = String::with_capacity(svg.len());
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() {
            let ch = svg[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
            continue;
        }
        let from = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let decimal = i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit();
        if !decimal {
            out.push_str(&svg[from..i]);
            continue;
        }
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        let v: f64 = svg[from..i].parse().unwrap();
        let r = format!("{:.1}", (v * 10.0).round() / 10.0);
        // no "-0.0"
        if r == "0.0" && out.ends_with('-') {
            out.pop();
        }
        out.push_str(&r);
    }
    out
}

/// The kinds of mark counted in a fingerprint.
const MARKS: [&str; 8] = [
    "path", "line", "polyline", "polygon", "rect", "circle", "text", "image",
];

/// FNV-1a, 64 bits: the same on every machine and toolchain.
fn hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// A mark with its `points` in one order whichever end or corner it was
/// drawn from: an open line starts from its smaller end, a closed outline
/// from its smallest corner, going towards the smaller neighbour.
fn canonical(mark: &str) -> String {
    let Some(at) = mark.find("points=\"") else {
        return mark.to_string();
    };
    let from = at + "points=\"".len();
    let Some(len) = mark[from..].find('"') else {
        return mark.to_string();
    };
    let mut pts: Vec<&str> = mark[from..from + len].split_whitespace().collect();
    let key = |p: &str| -> (i64, i64) {
        let mut it = p
            .split(',')
            .map(|v| (v.parse::<f64>().unwrap_or(0.0) * 10.0).round() as i64);
        (it.next().unwrap_or(0), it.next().unwrap_or(0))
    };
    let n = pts.len();
    if n > 1 && mark.starts_with("<polygon") {
        let first = (0..n).min_by_key(|&i| key(pts[i])).unwrap();
        pts.rotate_left(first);
        if key(pts[n - 1]) < key(pts[1]) {
            pts[1..].reverse();
        }
    } else if n > 1 && key(pts[n - 1]) < key(pts[0]) {
        pts.reverse();
    }
    format!("{}{}{}", &mark[..from], pts.join(" "), &mark[from + len..])
}

/// One line for a drawing: its hash, then each kind of mark it has.
fn fingerprint(svg: &str) -> String {
    let mut marks: Vec<String> = svg.lines().map(canonical).collect();
    marks.sort();
    let mut out = format!("{:016x}", hash(&marks.join("\n")));
    for mark in MARKS {
        let n =
            svg.matches(&format!("<{mark} ")).count() + svg.matches(&format!("<{mark}>")).count();
        if n > 0 {
            out.push_str(&format!(" {mark}={n}"));
        }
    }
    out
}

fn file_name(view: &str) -> String {
    let clean: String = view
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{clean}.svg")
}

/// Every view of `m` that draws, by name, as rounded SVG.
fn drawings(m: &Model) -> Vec<(String, String)> {
    let mut views: Vec<_> = m
        .iter()
        .filter_map(|e| match &e.kind {
            ElementKind::View(v) => Some((e.name.clone(), e.id, v.paper_scale())),
            _ => None,
        })
        .collect();
    views.sort_by(|a, b| a.0.cmp(&b.0));
    views
        .into_iter()
        .filter_map(|(name, id, scale)| {
            let d = creabim_geom::section::view_drawing(m, id)?;
            Some((name, rounded(&d.to_svg(scale))))
        })
        .collect()
}

fn check(model_name: &str, m: &Model) {
    let drawn = drawings(m);
    assert!(!drawn.is_empty(), "{model_name} has no views to draw");
    // what was drawn, to look at
    let out = actual_dir().join(model_name);
    std::fs::create_dir_all(&out).unwrap();
    let mut now = String::new();
    for (view, svg) in &drawn {
        std::fs::write(out.join(file_name(view)), svg).unwrap();
        now.push_str(&format!("{view} | {}\n", fingerprint(svg)));
    }
    let path = golden_dir().join(format!("{model_name}.txt"));
    if std::env::var_os("CREABIM_BLESS").is_some() {
        std::fs::create_dir_all(golden_dir()).unwrap();
        std::fs::write(&path, &now).unwrap();
        return;
    }
    let kept = std::fs::read_to_string(&path)
        .unwrap_or_else(|_| panic!("no kept fingerprints: {}", path.display()))
        .replace("\r\n", "\n");
    if kept == now {
        return;
    }
    let side = |text: &str| -> std::collections::BTreeMap<String, String> {
        text.lines()
            .filter_map(|l| l.split_once(" | "))
            .map(|(v, f)| (v.to_string(), f.to_string()))
            .collect()
    };
    let (kept, now) = (side(&kept), side(&now));
    let mut changes = vec![];
    for (view, was) in &kept {
        match now.get(view) {
            None => changes.push(format!("{view}: no longer drawn")),
            Some(is) if is != was => changes.push(format!("{view}:\n  kept: {was}\n  now:  {is}")),
            _ => {}
        }
    }
    for view in now.keys().filter(|v| !kept.contains_key(*v)) {
        changes.push(format!("{view}: a new drawing"));
    }
    panic!(
        "{model_name}: {} of {} drawings changed. What was drawn is in {}.\n\
         If the change is meant, look at it, then run with CREABIM_BLESS=1.\n\n{}",
        changes.len(),
        now.len(),
        out.display(),
        changes.join("\n")
    );
}

#[test]
fn starter_house_drawings_are_as_kept() {
    check("starter", &creabim_core::starter::starter_house());
}

#[test]
fn sample_house_drawings_are_as_kept() {
    check("sample", &creabim_core::sample::sample_house());
}

#[test]
fn decimals_round_and_the_rest_stays() {
    assert_eq!(
        rounded(r##"<path d="M1.234,-0.04L10 2.25" fill="#a3f2c1"/>W01 x2.5"##),
        r##"<path d="M1.2,0.0L10 2.3" fill="#a3f2c1"/>W01 x2.5"##
    );
}

#[test]
fn a_fingerprint_counts_marks() {
    let f = fingerprint(r#"<svg><path d=""/><path d=""/><text>a</text></svg>"#);
    assert!(f.ends_with(" path=2 text=1"), "{f}");
}

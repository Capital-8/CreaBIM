//! CAD hatch pattern files (`.pat`), as AutoCAD, Revit and most CAD read
//! and write them: a header line `*NAME, description`, then one line per
//! family of parallel lines, `angle, x, y, along, across, dash, gap...`
//! (gaps negative, 0 a dot). Revit adds `;%UNITS=MM` and `;%TYPE=MODEL`
//! (true size) or `;%TYPE=DRAFTING` (on paper).
//!
//! CreaBIM keeps paper patterns in millimetres and true-size ones in
//! metres. A recipe made of rectangles in courses (a bond, a grid of
//! tiles) is written as the line families that draw it; others (boards,
//! stones, drawn tiles) have no `.pat` form and are left out.

use crate::pattern::{Bond, Recipe};
use crate::styles::{HatchLine, HatchPattern, PatternUnits};
use glam::DVec2;

/// What reading a `.pat` file found.
#[derive(Debug, Default)]
pub struct PatRead {
    pub patterns: Vec<HatchPattern>,
    /// Lines that could not be read, with their line numbers.
    pub skipped: Vec<String>,
}

/// Read every pattern of a `.pat` file. `model` says what a file that does
/// not say its type holds: true-size (a Revit model pattern) or paper.
pub fn read(text: &str, model_by_default: bool) -> PatRead {
    let mut out = PatRead::default();
    // millimetres a file unit is
    let mut unit_mm = 1.0;
    let mut file_model = model_by_default;
    let mut cur: Option<(HatchPattern, Option<bool>)> = None;
    let finish = |cur: &mut Option<(HatchPattern, Option<bool>)>,
                  out: &mut PatRead,
                  unit_mm: f64,
                  file_model: bool| {
        if let Some((mut p, model)) = cur.take() {
            let model = model.unwrap_or(file_model);
            // to the pattern's units: mm on paper, m true size
            let k = if model { unit_mm / 1000.0 } else { unit_mm };
            p.units = if model {
                PatternUnits::Model
            } else {
                PatternUnits::Paper
            };
            for l in &mut p.lines {
                l.spacing *= k;
                l.offset *= k;
                l.shift *= k;
                l.phase *= k;
                for d in &mut l.dashes {
                    *d *= k;
                }
            }
            if !p.lines.is_empty() {
                out.patterns.push(p);
            }
        }
    };
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix(';') {
            let rest = rest.trim().to_ascii_uppercase().replace(' ', "");
            if let Some(u) = rest.strip_prefix("%UNITS=") {
                unit_mm = if u.starts_with("INCH") { 25.4 } else { 1.0 };
            } else if let Some(t) = rest.strip_prefix("%TYPE=") {
                let model = t.starts_with("MODEL");
                match &mut cur {
                    Some((_, m)) => *m = Some(model),
                    None => file_model = model,
                }
            }
            continue;
        }
        if let Some(head) = line.strip_prefix('*') {
            finish(&mut cur, &mut out, unit_mm, file_model);
            let name = head.split(',').next().unwrap_or("").trim();
            if name.is_empty() {
                out.skipped
                    .push(format!("line {}: a pattern with no name", n + 1));
                continue;
            }
            cur = Some((
                HatchPattern {
                    name: name.to_string(),
                    ..Default::default()
                },
                None,
            ));
            continue;
        }
        let Some((p, _)) = &mut cur else {
            out.skipped
                .push(format!("line {}: lines before any *NAME", n + 1));
            continue;
        };
        // a trailing ; comment on a data line
        let data = line.split(';').next().unwrap_or("");
        let v: Vec<f64> = data
            .split(',')
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .map_while(|t| t.parse::<f64>().ok())
            .collect();
        if v.len() < 5 || v.iter().any(|x| !x.is_finite()) {
            out.skipped
                .push(format!("line {}: not a line family", n + 1));
            continue;
        }
        match family(&v) {
            Some(l) => p.lines.push(l),
            None => out
                .skipped
                .push(format!("line {}: lines with no spacing", n + 1)),
        }
    }
    finish(&mut cur, &mut out, unit_mm, file_model);
    out
}

/// One `angle, x, y, along, across, dashes...` line as a family.
fn family(v: &[f64]) -> Option<HatchLine> {
    let (angle, x, y, along, across) = (v[0], v[1], v[2], v[3], v[4]);
    if across.abs() < 1e-12 {
        return None;
    }
    let dir = DVec2::from_angle(angle.to_radians());
    let o = DVec2::new(x, y);
    // lines run `across` apart; a negative step counts the other way
    let (spacing, shift) = if across < 0.0 {
        (-across, -along)
    } else {
        (across, along)
    };
    // dash, gap, dash...: CreaBIM's are all positive, a gap where PAT has
    // a negative number; two of a kind in a row get an empty one between
    let mut dashes = vec![];
    for d in &v[5..] {
        let gap = *d < 0.0;
        if (dashes.len() % 2 == 1) != gap {
            dashes.push(0.0);
        }
        dashes.push(d.abs());
    }
    if dashes.len() % 2 == 1 {
        dashes.push(0.0);
    }
    Some(HatchLine {
        angle,
        spacing,
        offset: o.dot(dir.perp()),
        shift,
        phase: o.dot(dir),
        dashes,
        along_longest: false,
    })
}

/// The line families that draw a pattern: its own, and its recipe's when
/// that has a line form. None when it has neither.
pub fn lines_of(p: &HatchPattern) -> Option<Vec<HatchLine>> {
    let mut v = p.lines.clone();
    if let Some(r) = &p.recipe {
        let mut more = recipe_lines(r)?;
        if p.angle != 0.0 {
            // turned about the origin: each family turns, its offsets with it
            for l in &mut more {
                l.angle += p.angle;
            }
        }
        v.append(&mut more);
    }
    (!v.is_empty()).then_some(v)
}

/// A recipe of rectangles in courses as the line families that draw their
/// outlines exactly; none for any other recipe.
pub fn recipe_lines(r: &Recipe) -> Option<Vec<HatchLine>> {
    // (unit length, unit height, joint, how far each course slides as a
    // share of a unit and its joint)
    let (l, h, j, slide) = match *r {
        Recipe::Bond {
            bond,
            length,
            height,
            joint,
        } => match bond {
            Bond::Stretcher => (length, height, joint, 0.5),
            Bond::Stack => (length, height, joint, 0.0),
            Bond::Third => (length, height, joint, 1.0 / 3.0),
            Bond::Flemish | Bond::English => return None,
        },
        Recipe::Tiles {
            width,
            height,
            joint,
            offset,
        } => (width, height, joint, offset.rem_euclid(1.0)),
        _ => return None,
    };
    let (pl, ph) = (l + j, h + j);
    // the slide as p/q of a unit
    let q = (1..=12).find(|q| ((slide * *q as f64) - (slide * *q as f64).round()).abs() < 1e-6)?;
    let p = (slide * q as f64).round() as i64;
    let mut out = vec![];
    // the courses' tops and bottoms, broken at the head joints
    let row = |y: f64| HatchLine {
        angle: 0.0,
        spacing: ph,
        offset: y,
        shift: slide * pl,
        phase: 0.0,
        dashes: if j > 0.0 { vec![l, j] } else { vec![] },
        along_longest: false,
    };
    out.push(row(0.0));
    if j > 0.0 {
        out.push(row(h));
    }
    // the head joints: upright lines pl/q apart, each dashed in the courses
    // whose joints fall on it. Upright, "across" runs toward -x.
    let pinv = (0..q as i64)
        .find(|x| (x * p).rem_euclid(q as i64) == 1 % q as i64)
        .unwrap_or(0);
    let step = ((q as i64 - pinv).rem_euclid(q as i64)) as f64 * ph;
    let head = |x: f64| HatchLine {
        angle: 90.0,
        spacing: pl / q as f64,
        offset: -x,
        shift: step,
        phase: 0.0,
        dashes: vec![h, q as f64 * ph - h],
        along_longest: false,
    };
    out.push(head(0.0));
    if j > 0.0 {
        out.push(head(l));
    }
    Some(out)
}

/// Write patterns as a `.pat` file (Revit style: millimetres, each with
/// its type). Returns the text and the names left out (no line form).
pub fn write(patterns: &[HatchPattern]) -> (String, Vec<String>) {
    let mut s = String::from(";%UNITS=MM\n;%VERSION=3.0\n;; Written by CreaBIM\n");
    let mut left = vec![];
    for p in patterns {
        let Some(lines) = lines_of(p) else {
            left.push(p.name.clone());
            continue;
        };
        let model = p.units == PatternUnits::Model;
        let k = if model { 1000.0 } else { 1.0 };
        let name: String = p
            .name
            .chars()
            .map(|c| if c == ',' || c == '*' { ' ' } else { c })
            .collect();
        s.push_str(&format!(
            "*{}, {}\n;%TYPE={}\n",
            name.trim(),
            name.trim(),
            if model { "MODEL" } else { "DRAFTING" }
        ));
        for l in lines {
            let dir = DVec2::from_angle(l.angle.to_radians());
            let o = dir.perp() * l.offset + dir * l.phase;
            let mut row = vec![
                num(l.angle),
                num(o.x * k),
                num(o.y * k),
                num(l.shift * k),
                num(l.spacing * k),
            ];
            for (i, d) in l.dashes.iter().enumerate() {
                if i % 2 == 0 {
                    row.push(num(d * k));
                } else if *d > 0.0 {
                    row.push(num(-d * k));
                }
            }
            s.push_str(&row.join(", "));
            s.push('\n');
        }
    }
    (s, left)
}

fn num(v: f64) -> String {
    let r = (v * 1e6).round() / 1e6;
    if r == 0.0 {
        "0".into()
    } else {
        format!("{r}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ACAD: &str = "\
;; a comment
*ANSI31, ANSI Iron, Brick, Stone masonry
45, 0,0, 0,3.175
*BRICK, Brick or masonry-type surface
0, 0,0, 0,6.35
90, 0,0, 6.35,6.35, 6.35,-6.35
";

    #[test]
    fn reads_autocad_paper_patterns() {
        let r = read(ACAD, false);
        assert!(r.skipped.is_empty(), "{:?}", r.skipped);
        assert_eq!(r.patterns.len(), 2);
        let a = &r.patterns[0];
        assert_eq!((a.name.as_str(), a.units), ("ANSI31", PatternUnits::Paper));
        assert_eq!(a.lines[0].angle, 45.0);
        assert!((a.lines[0].spacing - 3.175).abs() < 1e-9);
        let b = &r.patterns[1];
        assert_eq!(b.lines[1].dashes, vec![6.35, 6.35]);
        assert!((b.lines[1].shift - 6.35).abs() < 1e-9);
    }

    #[test]
    fn reads_revit_model_patterns_in_metres() {
        let text =
            ";%UNITS=MM\n*Block 400x200\n;%TYPE=MODEL\n0, 0,0, 0,200\n90, 0,0, 200,200, 200,-200\n";
        let r = read(text, false);
        let p = &r.patterns[0];
        assert_eq!(p.units, PatternUnits::Model);
        assert!((p.lines[0].spacing - 0.2).abs() < 1e-12);
        assert_eq!(p.lines[1].dashes, vec![0.2, 0.2]);
        // inches
        let r = read(";%UNITS=INCH\n*X\n;%TYPE=MODEL\n0, 0,0, 0,1\n", false);
        assert!((r.patterns[0].lines[0].spacing - 0.0254).abs() < 1e-12);
    }

    #[test]
    fn bad_lines_are_reported_not_fatal() {
        let r = read("0,0,0,0,1\n*A\nnonsense\n0,0,0,0,0\n45,0,0,0,2\n", false);
        assert_eq!(r.patterns.len(), 1);
        assert_eq!(r.skipped.len(), 3);
    }

    #[test]
    fn written_patterns_read_back_the_same() {
        let r = read(ACAD, false);
        let (text, left) = write(&r.patterns);
        assert!(left.is_empty());
        let back = read(&text, true);
        assert_eq!(back.patterns.len(), 2);
        for (a, b) in r.patterns.iter().zip(&back.patterns) {
            assert_eq!(a.name, b.name);
            assert_eq!(a.units, b.units);
            assert_eq!(a.lines.len(), b.lines.len());
            for (x, y) in a.lines.iter().zip(&b.lines) {
                assert!((x.spacing - y.spacing).abs() < 1e-6);
                assert!((x.offset - y.offset).abs() < 1e-6);
                assert!((x.shift - y.shift).abs() < 1e-6);
                assert_eq!(x.dashes.len(), y.dashes.len());
            }
        }
    }

    #[test]
    fn a_bond_writes_as_lines_and_a_drawn_tile_is_left_out() {
        let bond = HatchPattern::from_recipe(
            "Brick",
            Recipe::Bond {
                bond: Bond::Stretcher,
                length: 0.215,
                height: 0.065,
                joint: 0.01,
            },
        );
        let drawn = HatchPattern::from_recipe("Leaf", crate::pattern::leaf_and_wave());
        let (text, left) = write(&[bond, drawn]);
        assert_eq!(left, vec!["Leaf".to_string()]);
        assert!(text.contains("*Brick"));
        assert!(text.contains(";%TYPE=MODEL"));
        let back = read(&text, false);
        // two course lines, two head-joint lines
        assert_eq!(back.patterns[0].lines.len(), 4);
        assert_eq!(back.patterns[0].units, PatternUnits::Model);
    }
}

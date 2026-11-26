//! Reading a survey: what a surveyor hands over as a table of points (a
//! spreadsheet, a CSV or a text file) or as a DXF drawing, turned into
//! the spot heights, contours and break lines of a terrain.
//!
//! A table is read as text cells first, so the caller can show it and let
//! the columns be picked; a guess at them is made from the header or from
//! what the columns hold. A DXF is read for what carries a height: points,
//! polylines at an elevation (contours) and 3D polylines (break lines).

use creabim_core::{GroundLine, GroundLineKind};
use glam::{DVec2, DVec3};

/// A table of cells as text, with its header row if it has one.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub header: Option<Vec<String>>,
    pub rows: Vec<Vec<String>>,
}

/// Which column holds what. Columns count from 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Columns {
    /// Easting.
    pub x: usize,
    /// Northing.
    pub y: usize,
    /// Height.
    pub z: usize,
    /// The point's code ("KERB", "TREE"), if the table has one.
    pub code: Option<usize>,
}

/// One surveyed point.
#[derive(Clone, Debug, PartialEq)]
pub struct SurveyPoint {
    pub at: DVec3,
    pub code: String,
}

impl Table {
    pub fn columns(&self) -> usize {
        let head = self.header.as_ref().map_or(0, |h| h.len());
        self.rows
            .iter()
            .map(|r| r.len())
            .max()
            .unwrap_or(0)
            .max(head)
    }

    /// A column's name: its header, or its letter.
    pub fn name(&self, col: usize) -> String {
        match self.header.as_ref().and_then(|h| h.get(col)) {
            Some(h) if !h.trim().is_empty() => h.trim().to_string(),
            _ => ((b'A' + (col % 26) as u8) as char).to_string(),
        }
    }
}

fn number(cell: &str) -> Option<f64> {
    let t = cell.trim();
    // "512 330.412" and "512,330.412" are thousands, "12,384" alone in a
    // cell of a semicolon file is a decimal comma
    let t = t.replace(['\u{a0}', ' ', '\''], "");
    let t = if t.contains(',') && !t.contains('.') && t.matches(',').count() == 1 {
        t.replace(',', ".")
    } else {
        t.replace(',', "")
    };
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A table from text: comma, semicolon, tab or space separated, as a CSV,
/// a surveyor's TXT or cells pasted from a spreadsheet are.
pub fn table_from_text(text: &str) -> Table {
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .collect();
    // the separator most lines agree on
    let split = |l: &str, sep: Option<char>| -> Vec<String> {
        match sep {
            Some(c) => l
                .split(c)
                .map(|s| s.trim().trim_matches('"').to_string())
                .collect(),
            None => l.split_whitespace().map(str::to_string).collect(),
        }
    };
    let sep = [Some('\t'), Some(';'), Some(','), None]
        .into_iter()
        .find(|sep| {
            let counts: Vec<usize> = lines
                .iter()
                .take(20)
                .map(|l| split(l, *sep).len())
                .collect();
            counts.first().is_some_and(|n| *n >= 3) && counts.iter().all(|n| *n == counts[0])
        })
        .unwrap_or(if lines.first().is_some_and(|l| l.contains(';')) {
            Some(';')
        } else if lines.first().is_some_and(|l| l.contains('\t')) {
            Some('\t')
        } else if lines.first().is_some_and(|l| l.contains(',')) {
            Some(',')
        } else {
            None
        });
    with_header(lines.iter().map(|l| split(l, sep)).collect())
}

/// Rows, the first taken as the header when it holds no numbers where
/// the rows under it do.
pub fn with_header(mut rows: Vec<Vec<String>>) -> Table {
    rows.retain(|r| r.iter().any(|c| !c.trim().is_empty()));
    let numeric = |r: &Vec<String>| r.iter().filter(|c| number(c).is_some()).count();
    let header =
        (rows.len() > 1 && numeric(&rows[0]) < 2 && numeric(&rows[1]) >= 3).then(|| rows.remove(0));
    Table { header, rows }
}

/// A table from a file: a spreadsheet (xlsx, xls, ods: its first sheet)
/// or text.
pub fn table_from_file(path: &std::path::Path) -> Result<Table, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if matches!(ext.as_str(), "xlsx" | "xlsm" | "xls" | "ods") {
        use calamine::Reader;
        let mut book = calamine::open_workbook_auto(path)
            .map_err(|e| format!("couldn't read {}: {e}", path.display()))?;
        let sheet = book
            .worksheet_range_at(0)
            .ok_or("the workbook has no sheets")?
            .map_err(|e| format!("couldn't read the first sheet: {e}"))?;
        let rows = sheet
            .rows()
            .map(|r| r.iter().map(|c| c.to_string()).collect())
            .collect();
        return Ok(with_header(rows));
    }
    let bytes =
        std::fs::read(path).map_err(|e| format!("couldn't read {}: {e}", path.display()))?;
    Ok(table_from_text(&String::from_utf8_lossy(&bytes)))
}

/// A guess at the columns: from the header's names, else from what the
/// columns hold (number, easting, northing, height, code).
pub fn guess_columns(t: &Table) -> Option<Columns> {
    let n = t.columns();
    if let Some(h) = &t.header {
        let find = |names: &[&str]| {
            h.iter().position(|c| {
                let c = c.trim().to_ascii_lowercase();
                names.iter().any(|n| {
                    c == *n || c.starts_with(&format!("{n} ")) || c.starts_with(&format!("{n}("))
                })
            })
        };
        let x = find(&["x", "e", "east", "easting", "eastings", "est"]);
        let y = find(&["y", "n", "north", "northing", "northings", "nord"]);
        let z = find(&[
            "z",
            "h",
            "elev",
            "elevation",
            "level",
            "height",
            "rl",
            "alt",
            "altitude",
            "cote",
        ]);
        if let (Some(x), Some(y), Some(z)) = (x, y, z) {
            let code = find(&["code", "desc", "description", "feature", "remark", "type"]);
            return Some(Columns { x, y, z, code });
        }
    }
    // the columns that hold numbers in (nearly) every row
    let rows = t.rows.len().max(1);
    let numeric: Vec<bool> = (0..n)
        .map(|c| {
            let hits = t
                .rows
                .iter()
                .filter(|r| r.get(c).is_some_and(|v| number(v).is_some()))
                .count();
            hits * 10 >= rows * 9
        })
        .collect();
    let whole = |c: usize| {
        t.rows
            .iter()
            .filter_map(|r| r.get(c).and_then(|v| number(v)))
            .all(|v| v.fract() == 0.0)
    };
    let mut nums: Vec<usize> = (0..n).filter(|c| numeric[*c]).collect();
    // a leading column of whole numbers is the point number
    if nums.len() >= 4 && whole(nums[0]) {
        nums.remove(0);
    }
    if nums.len() < 3 {
        return None;
    }
    let code = (0..n).rev().find(|c| !numeric[*c] && *c > nums[0]);
    Some(Columns {
        x: nums[0],
        y: nums[1],
        z: nums[2],
        code,
    })
}

/// The table's points: `unit` metres to one of its units. Rows that
/// don't hold three numbers are left out; their count comes back too.
pub fn points(t: &Table, cols: Columns, unit: f64) -> (Vec<SurveyPoint>, usize) {
    let mut out = vec![];
    let mut skipped = 0;
    for r in &t.rows {
        let cell = |c: usize| r.get(c).and_then(|v| number(v));
        match (cell(cols.x), cell(cols.y), cell(cols.z)) {
            (Some(x), Some(y), Some(z)) => out.push(SurveyPoint {
                at: DVec3::new(x, y, z) * unit,
                code: cols
                    .code
                    .and_then(|c| r.get(c))
                    .map(|c| c.trim().to_string())
                    .unwrap_or_default(),
            }),
            _ => skipped += 1,
        }
    }
    (out, skipped)
}

/// The first word of a code, upper case: "kerb 2" is KERB.
pub fn code_word(code: &str) -> String {
    code.split_whitespace()
        .next()
        .unwrap_or("")
        .trim_end_matches(|c: char| c.is_ascii_digit())
        .to_ascii_uppercase()
}

/// The codes the points carry, most used first, with their counts.
pub fn codes(points: &[SurveyPoint]) -> Vec<(String, usize)> {
    let mut out: Vec<(String, usize)> = vec![];
    for p in points {
        let w = code_word(&p.code);
        if w.is_empty() {
            continue;
        }
        match out.iter_mut().find(|c| c.0 == w) {
            Some(c) => c.1 += 1,
            None => out.push((w, 1)),
        }
    }
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

/// Codes that usually name a surveyed string.
pub fn is_line_code(word: &str) -> bool {
    const LINES: [&str; 22] = [
        "KERB", "KB", "CURB", "WALL", "TOP", "TOB", "BOT", "BOB", "TOE", "BANK", "DITCH", "EDGE",
        "EOP", "EP", "CL", "FENCE", "FL", "BL", "BRK", "BREAK", "RIDGE", "CH",
    ];
    LINES.contains(&word)
}

/// The points as a terrain's data: every point a spot height, and the
/// runs of points sharing one of the `line_codes` joined in the order
/// surveyed as break lines (a code's own number, as in "KERB2", keeps
/// two strings of one kind apart).
pub fn assemble(points: &[SurveyPoint], line_codes: &[String]) -> (Vec<DVec3>, Vec<GroundLine>) {
    let spots = points.iter().map(|p| p.at).collect();
    let mut strings: Vec<(String, Vec<DVec3>)> = vec![];
    for p in points {
        let word = code_word(&p.code);
        if word.is_empty() || !line_codes.contains(&word) {
            continue;
        }
        let name = p
            .code
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        match strings.iter_mut().find(|s| s.0 == name) {
            Some(s) => s.1.push(p.at),
            None => strings.push((name, vec![p.at])),
        }
    }
    let lines = strings
        .into_iter()
        .filter(|s| s.1.len() >= 2)
        .map(|s| GroundLine {
            kind: GroundLineKind::Break,
            points: s.1,
            closed: false,
        })
        .collect();
    (spots, lines)
}

/// The outline round `pts` (their convex hull, counter-clockwise),
/// pushed `margin` out.
pub fn outline_around(pts: &[DVec2], margin: f64) -> Vec<DVec2> {
    let mut p: Vec<DVec2> = pts.to_vec();
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    p.dedup_by(|a, b| a.distance(*b) < 1e-6);
    if p.len() < 3 {
        return vec![];
    }
    let mut hull: Vec<DVec2> = vec![];
    for pass in 0..2 {
        let start = hull.len();
        let order: Box<dyn Iterator<Item = &DVec2>> = if pass == 0 {
            Box::new(p.iter())
        } else {
            Box::new(p.iter().rev())
        };
        for q in order {
            while hull.len() >= start + 2 {
                let (a, b) = (hull[hull.len() - 2], hull[hull.len() - 1]);
                if (b - a).perp_dot(*q - b) <= 1e-12 {
                    hull.pop();
                } else {
                    break;
                }
            }
            hull.push(*q);
        }
        hull.pop();
    }
    if hull.len() < 3 || margin <= 0.0 {
        return hull;
    }
    // each corner out along the bisector of its two edges
    let n = hull.len();
    (0..n)
        .map(|i| {
            let (a, b, c) = (hull[(i + n - 1) % n], hull[i], hull[(i + 1) % n]);
            let n1 = -(b - a).normalize_or_zero().perp();
            let n2 = -(c - b).normalize_or_zero().perp();
            let m = (n1 + n2).normalize_or_zero();
            b + m * (margin / m.dot(n1).max(0.3))
        })
        .collect()
}

/// The middle of the data's extent, to the whole metre: what comes off
/// a survey's coordinates to bring it to the drawing's origin.
pub fn centre(spots: &[DVec3], lines: &[GroundLine]) -> Option<DVec2> {
    let mut all = spots
        .iter()
        .chain(lines.iter().flat_map(|l| l.points.iter()))
        .map(|p| p.truncate());
    let first = all.next()?;
    let (lo, hi) = all.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
    Some(((lo + hi) / 2.0).round())
}

/// The lowest and highest of the data.
pub fn heights(spots: &[DVec3], lines: &[GroundLine]) -> Option<(f64, f64)> {
    let mut all = spots
        .iter()
        .chain(lines.iter().flat_map(|l| l.points.iter()))
        .map(|p| p.z);
    let first = all.next()?;
    Some(all.fold((first, first), |(lo, hi), z| (lo.min(z), hi.max(z))))
}

/// Take `shift` off every point.
pub fn shift(spots: &mut [DVec3], lines: &mut [GroundLine], shift: DVec3) {
    let lines = lines.iter_mut().flat_map(|l| l.points.iter_mut());
    for p in spots.iter_mut().chain(lines) {
        *p -= shift;
    }
}

/// A contour interval that gives a ground rising `range` ten to twenty
/// lines.
pub fn contour_interval(range: f64) -> f64 {
    const STEPS: [f64; 9] = [0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0];
    STEPS
        .into_iter()
        .find(|s| range / s <= 20.0)
        .unwrap_or(50.0)
}

// ---- DXF ----

/// A polyline of a DXF drawing.
#[derive(Clone, Debug, PartialEq)]
pub struct DxfLine {
    pub layer: String,
    pub points: Vec<DVec3>,
    pub closed: bool,
}

impl DxfLine {
    /// At one height off zero: a contour.
    pub fn is_contour(&self) -> bool {
        let z = self.points.first().map_or(0.0, |p| p.z);
        z.abs() > 1e-9 && self.points.iter().all(|p| (p.z - z).abs() < 1e-6)
    }

    /// Its heights vary: a surveyed string.
    pub fn is_break(&self) -> bool {
        let z = self.points.first().map_or(0.0, |p| p.z);
        self.points.iter().any(|p| (p.z - z).abs() >= 1e-6)
    }
}

/// What a DXF drawing holds of a survey, in its own units.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DxfSurvey {
    /// Metres to one drawing unit, when the drawing says ($INSUNITS).
    pub unit: Option<f64>,
    /// Points with a height, and their layer.
    pub points: Vec<(String, DVec3)>,
    pub lines: Vec<DxfLine>,
}

/// What a layer of the drawing would give.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DxfLayer {
    pub name: String,
    pub points: usize,
    pub contours: usize,
    pub breaks: usize,
    /// Lines lying at zero: plain drawing, no heights.
    pub flat: usize,
}

impl DxfSurvey {
    /// The drawing's layers that hold anything, by name.
    pub fn layers(&self) -> Vec<DxfLayer> {
        let mut out: Vec<DxfLayer> = vec![];
        let mut at = |name: &str| -> usize {
            match out.iter().position(|l| l.name == name) {
                Some(i) => i,
                None => {
                    out.push(DxfLayer {
                        name: name.to_string(),
                        ..Default::default()
                    });
                    out.len() - 1
                }
            }
        };
        let mut hits = vec![];
        for (l, _) in &self.points {
            hits.push((at(l), 0));
        }
        for l in &self.lines {
            let kind = if l.is_contour() {
                1
            } else if l.is_break() {
                2
            } else {
                3
            };
            hits.push((at(&l.layer), kind));
        }
        for (i, kind) in hits {
            match kind {
                0 => out[i].points += 1,
                1 => out[i].contours += 1,
                2 => out[i].breaks += 1,
                _ => out[i].flat += 1,
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    /// The data of the layers `on` picks, `unit` metres to a drawing
    /// unit: points with a height as spot heights, polylines at one
    /// height as contours, 3D polylines as break lines.
    pub fn ground(&self, on: impl Fn(&str) -> bool, unit: f64) -> (Vec<DVec3>, Vec<GroundLine>) {
        let spots = self
            .points
            .iter()
            .filter(|(l, _)| on(l))
            .map(|(_, p)| *p * unit)
            .collect();
        let lines = self
            .lines
            .iter()
            .filter(|l| on(&l.layer) && (l.is_contour() || l.is_break()) && l.points.len() >= 2)
            .map(|l| GroundLine {
                kind: if l.is_contour() {
                    GroundLineKind::Contour
                } else {
                    GroundLineKind::Break
                },
                points: l.points.iter().map(|p| *p * unit).collect(),
                closed: l.closed,
            })
            .collect();
        (spots, lines)
    }
}

/// Read a DXF drawing's points and polylines with their heights.
pub fn dxf_survey(text: &str) -> Result<DxfSurvey, String> {
    // a DXF is pairs of lines: a group code, then its value
    let mut pairs: Vec<(i32, &str)> = vec![];
    let mut lines = text.lines();
    while let (Some(code), Some(value)) = (lines.next(), lines.next()) {
        let Ok(code) = code.trim().parse::<i32>() else {
            return Err("this doesn't read as a text DXF (a binary DXF or a DWG must be saved as DXF first)".into());
        };
        pairs.push((code, value.trim()));
    }
    let mut out = DxfSurvey::default();
    let num = |v: &str| v.parse::<f64>().unwrap_or(0.0);
    // the header's units
    for w in pairs.windows(2) {
        if w[0] == (9, "$INSUNITS") {
            out.unit = match w[1].1.parse::<i32>().unwrap_or(0) {
                1 => Some(0.0254),
                2 => Some(0.3048),
                4 => Some(0.001),
                5 => Some(0.01),
                6 => Some(1.0),
                _ => None,
            };
        }
    }
    let start = pairs
        .windows(2)
        .position(|w| w[0] == (0, "SECTION") && w[1] == (2, "ENTITIES"))
        .ok_or("the DXF has no drawing in it (no ENTITIES section)")?;
    let mut i = start + 2;
    // an old-style polyline being read: its vertices follow it
    let mut poly: Option<DxfLine> = None;
    while i < pairs.len() {
        let (code, kind) = pairs[i];
        if code != 0 {
            i += 1;
            continue;
        }
        let end = (i + 1..pairs.len())
            .find(|j| pairs[*j].0 == 0)
            .unwrap_or(pairs.len());
        let body = &pairs[i + 1..end];
        let get = |c: i32| body.iter().find(|p| p.0 == c).map(|p| p.1);
        let layer = get(8).unwrap_or("0").to_string();
        let flags = get(70).and_then(|v| v.parse::<i32>().ok()).unwrap_or(0);
        match kind {
            "ENDSEC" => break,
            "POINT" => {
                let p = DVec3::new(
                    num(get(10).unwrap_or("0")),
                    num(get(20).unwrap_or("0")),
                    num(get(30).unwrap_or("0")),
                );
                if p.z.abs() > 1e-9 {
                    out.points.push((layer, p));
                }
            }
            "LINE" => {
                let p = |x: i32, y: i32, z: i32| {
                    DVec3::new(
                        num(get(x).unwrap_or("0")),
                        num(get(y).unwrap_or("0")),
                        num(get(z).unwrap_or("0")),
                    )
                };
                out.lines.push(DxfLine {
                    layer,
                    points: vec![p(10, 20, 30), p(11, 21, 31)],
                    closed: false,
                });
            }
            "LWPOLYLINE" => {
                let z = num(get(38).unwrap_or("0"));
                let mut pts = vec![];
                let mut x = None;
                for (c, v) in body {
                    match c {
                        10 => x = Some(num(v)),
                        20 => {
                            if let Some(x) = x.take() {
                                pts.push(DVec3::new(x, num(v), z));
                            }
                        }
                        _ => {}
                    }
                }
                out.lines.push(DxfLine {
                    layer,
                    points: pts,
                    closed: flags & 1 != 0,
                });
            }
            "POLYLINE" => {
                poly = Some(DxfLine {
                    layer,
                    // a 2D polyline's own elevation; a 3D one's vertices
                    // carry theirs
                    points: vec![DVec3::new(0.0, 0.0, num(get(30).unwrap_or("0")))],
                    closed: flags & 1 != 0,
                });
            }
            "VERTEX" => {
                if let Some(p) = &mut poly {
                    let base = p.points[0].z;
                    let z = get(30).map(num).filter(|z| z.abs() > 1e-12).unwrap_or(base);
                    p.points.push(DVec3::new(
                        num(get(10).unwrap_or("0")),
                        num(get(20).unwrap_or("0")),
                        z,
                    ));
                }
            }
            "SEQEND" => {
                if let Some(mut p) = poly.take() {
                    p.points.remove(0);
                    out.lines.push(p);
                }
            }
            _ => {}
        }
        i = end;
    }
    out.lines.retain(|l| l.points.len() >= 2);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_csv_with_a_header_gives_its_points_and_codes() {
        let t = table_from_text(
            "Point,Easting,Northing,Level,Code\n101,512330.412,4362118.090,12.384,GL\n102,512335.020,4362119.774,12.511,KERB\n103,512338.950,4362121.300,12.640,KERB\n104,x,y,z,bad\n",
        );
        assert_eq!(t.header.as_ref().unwrap()[1], "Easting");
        let c = guess_columns(&t).unwrap();
        assert_eq!((c.x, c.y, c.z, c.code), (1, 2, 3, Some(4)));
        let (p, skipped) = points(&t, c, 1.0);
        assert_eq!((p.len(), skipped), (3, 1));
        assert_eq!(p[0].at, DVec3::new(512330.412, 4362118.090, 12.384));
        assert_eq!(
            codes(&p),
            vec![("KERB".to_string(), 2), ("GL".to_string(), 1)]
        );
        let (spots, lines) = assemble(&p, &["KERB".to_string()]);
        assert_eq!(spots.len(), 3);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].kind, GroundLineKind::Break);
        assert_eq!(lines[0].points.len(), 2);
    }

    #[test]
    fn a_bare_text_file_is_read_by_what_its_columns_hold() {
        // number, easting, northing, height, separated by spaces
        let t = table_from_text("1 10.0 20.0 1.5\n2 12.5 20.0 1.7\n3 12.5 24.0 2.1\n");
        assert!(t.header.is_none());
        let c = guess_columns(&t).unwrap();
        assert_eq!((c.x, c.y, c.z, c.code), (1, 2, 3, None));
        // x y z alone
        let t = table_from_text("10.0\t20.0\t1.5\n12.5\t20.0\t1.7\n");
        let c = guess_columns(&t).unwrap();
        assert_eq!((c.x, c.y, c.z), (0, 1, 2));
        // semicolons with decimal commas, in millimetres
        let t = table_from_text("X;Y;Z\n1000,5;2000;150\n3000;2000;175\n");
        let (p, _) = points(&t, guess_columns(&t).unwrap(), 0.001);
        assert!((p[0].at - DVec3::new(1.0005, 2.0, 0.15)).length() < 1e-12);
    }

    #[test]
    fn strings_of_one_kind_keep_apart_by_their_number() {
        let pt = |x: f64, code: &str| SurveyPoint {
            at: DVec3::new(x, 0.0, 1.0),
            code: code.to_string(),
        };
        let p = [
            pt(0.0, "KERB1"),
            pt(1.0, "KERB2"),
            pt(2.0, "KERB1"),
            pt(3.0, "KERB2"),
            pt(4.0, "TREE 0.4"),
        ];
        let (_, lines) = assemble(&p, &["KERB".to_string()]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].points[1].x, 2.0);
        assert!(is_line_code("KERB") && !is_line_code("TREE"));
    }

    #[test]
    fn the_outline_round_points_holds_them_all() {
        let pts = [
            DVec2::new(0.0, 0.0),
            DVec2::new(10.0, 0.0),
            DVec2::new(10.0, 8.0),
            DVec2::new(0.0, 8.0),
            DVec2::new(5.0, 4.0),
        ];
        let tight = outline_around(&pts, 0.0);
        assert_eq!(tight.len(), 4);
        assert!(creabim_core::polygon::signed_area2(&tight) > 0.0);
        let wide = outline_around(&pts, 1.0);
        for p in pts {
            assert!(creabim_core::polygon::contains(&wide, p));
        }
        assert!((creabim_core::polygon::signed_area2(&wide) / 2.0 - 120.0).abs() < 1e-6);
    }

    #[test]
    fn a_survey_far_from_zero_comes_to_the_origin() {
        let mut spots = vec![
            DVec3::new(512330.4, 4362118.1, 312.4),
            DVec3::new(512390.4, 4362158.1, 318.9),
        ];
        let mut lines = vec![];
        let c = centre(&spots, &lines).unwrap();
        assert_eq!(c, DVec2::new(512360.0, 4362138.0));
        assert_eq!(heights(&spots, &lines), Some((312.4, 318.9)));
        shift(&mut spots, &mut lines, c.extend(312.0));
        assert!((spots[0] - DVec3::new(-29.6, -19.9, 0.4)).length() < 1e-6);
        assert_eq!(contour_interval(6.5), 0.5);
        assert_eq!(contour_interval(0.9), 0.05);
        assert_eq!(contour_interval(40.0), 2.0);
    }

    #[test]
    fn a_dxf_gives_contours_break_lines_and_points_by_layer() {
        let dxf = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n6\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n\
0\nLWPOLYLINE\n8\nCONTOURS\n90\n3\n70\n0\n38\n12.5\n10\n0\n20\n0\n10\n5\n20\n1\n10\n10\n20\n0\n\
0\nPOLYLINE\n8\nKERB\n66\n1\n70\n8\n10\n0\n20\n0\n30\n0\n\
0\nVERTEX\n8\nKERB\n10\n0\n20\n5\n30\n12.1\n0\nVERTEX\n8\nKERB\n10\n10\n20\n5\n30\n12.4\n0\nSEQEND\n\
0\nPOINT\n8\nSPOTS\n10\n3\n20\n3\n30\n12.75\n\
0\nLINE\n8\nBUILDING\n10\n0\n20\n0\n30\n0\n11\n4\n21\n0\n31\n0\n\
0\nENDSEC\n0\nEOF\n";
        let s = dxf_survey(dxf).unwrap();
        assert_eq!(s.unit, Some(1.0));
        let layers = s.layers();
        let of = |n: &str| layers.iter().find(|l| l.name == n).unwrap().clone();
        assert_eq!(of("CONTOURS").contours, 1);
        assert_eq!(of("KERB").breaks, 1);
        assert_eq!(of("SPOTS").points, 1);
        assert_eq!(of("BUILDING").flat, 1);
        let (spots, lines) = s.ground(|_| true, 1.0);
        assert_eq!(spots, vec![DVec3::new(3.0, 3.0, 12.75)]);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].kind, GroundLineKind::Contour);
        assert_eq!(lines[0].points[1], DVec3::new(5.0, 1.0, 12.5));
        assert_eq!(lines[1].kind, GroundLineKind::Break);
        assert!(dxf_survey("not a dxf\nat all\n").is_err());
    }
}

//! Map data as ground data: LandXML surfaces, height grids (Esri ASCII
//! grid, GeoTIFF), GeoJSON and shapefiles. Each reads into the same
//! layered survey a DXF drawing does ([`DxfSurvey`]): points with a
//! height, lines at one height (contours), lines with heights (break
//! lines) and lines without, by layer.
//!
//! Coordinates stay in the file's map coordinate system. Longitude and
//! latitude (WGS 84, what GeoJSON is written in) are projected to the UTM
//! zone they fall in, so that everything is in metres. Nothing is
//! reprojected from one projected system to another.

use crate::survey::{DxfLine, DxfSurvey};
use glam::DVec3;
use std::path::Path;

/// A height grid is thinned to about this many points: the ground is
/// built from them as from a survey, and a terrain of a million points
/// is no faster to read on a drawing.
pub const MAX_GRID_POINTS: usize = 12_000;

/// What a map file holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapData {
    pub survey: DxfSurvey,
    /// Its coordinate system, when the file says: "EPSG:32617" or a name.
    pub crs: Option<String>,
    /// What was done to it on the way in, for the person to read.
    pub notes: Vec<String>,
}

/// Whether a file is one of these, by its name and how it starts.
pub fn is_map_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "xml" | "landxml" | "tif" | "tiff" | "geojson" | "json" | "shp" | "osm" => true,
        "asc" | "grd" => std::fs::read(path).is_ok_and(|b| is_ascii_grid(&b)),
        _ => false,
    }
}

fn is_ascii_grid(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(64)]).to_ascii_lowercase();
    head.trim_start().starts_with("ncols")
}

/// Read a map file.
pub fn read(path: &Path) -> Result<MapData, String> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let bytes =
        || std::fs::read(path).map_err(|e| format!("couldn't read {}: {e}", path.display()));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "Layer".to_string());
    match ext.as_str() {
        "xml" | "landxml" => landxml(&String::from_utf8_lossy(&bytes()?)),
        "tif" | "tiff" => geotiff(std::io::Cursor::new(bytes()?)),
        "geojson" | "json" => geojson(&String::from_utf8_lossy(&bytes()?)),
        "osm" => osm(&String::from_utf8_lossy(&bytes()?)),
        "shp" => {
            let dbf = std::fs::read(path.with_extension("dbf")).ok();
            let prj = std::fs::read_to_string(path.with_extension("prj")).ok();
            shapefile(&bytes()?, dbf.as_deref(), prj.as_deref(), &stem)
        }
        _ => ascii_grid(&String::from_utf8_lossy(&bytes()?)),
    }
}

// ---- longitude and latitude to metres ----

/// The UTM zone a longitude falls in, 1 to 60.
pub fn utm_zone(longitude: f64) -> u32 {
    ((((longitude + 180.0) / 6.0).floor() as i64).rem_euclid(60) + 1) as u32
}

/// WGS 84 longitude and latitude, degrees, as easting and northing in a
/// UTM zone, metres; `south` adds the southern hemisphere's ten million.
pub fn utm(longitude: f64, latitude: f64, zone: u32, south: bool) -> (f64, f64) {
    let a = 6_378_137.0_f64;
    let f = 1.0 / 298.257_223_563;
    let k0 = 0.9996;
    let e2 = f * (2.0 - f);
    let ep2 = e2 / (1.0 - e2);
    let phi = latitude.to_radians();
    let lam0 = ((zone as f64 - 1.0) * 6.0 - 180.0 + 3.0).to_radians();
    let (s, c, t) = (phi.sin(), phi.cos(), phi.tan());
    let n = a / (1.0 - e2 * s * s).sqrt();
    let t2 = t * t;
    let cc = ep2 * c * c;
    let aa = (longitude.to_radians() - lam0) * c;
    let (e4, e6) = (e2 * e2, e2 * e2 * e2);
    let m = a
        * ((1.0 - e2 / 4.0 - 3.0 * e4 / 64.0 - 5.0 * e6 / 256.0) * phi
            - (3.0 * e2 / 8.0 + 3.0 * e4 / 32.0 + 45.0 * e6 / 1024.0) * (2.0 * phi).sin()
            + (15.0 * e4 / 256.0 + 45.0 * e6 / 1024.0) * (4.0 * phi).sin()
            - (35.0 * e6 / 3072.0) * (6.0 * phi).sin());
    let east = k0
        * n
        * (aa
            + (1.0 - t2 + cc) * aa.powi(3) / 6.0
            + (5.0 - 18.0 * t2 + t2 * t2 + 72.0 * cc - 58.0 * ep2) * aa.powi(5) / 120.0)
        + 500_000.0;
    let north = k0
        * (m + n
            * t
            * (aa * aa / 2.0
                + (5.0 - t2 + 9.0 * cc + 4.0 * cc * cc) * aa.powi(4) / 24.0
                + (61.0 - 58.0 * t2 + t2 * t2 + 600.0 * cc - 330.0 * ep2) * aa.powi(6) / 720.0));
    (east, if south { north + 10_000_000.0 } else { north })
}

fn each_point(s: &mut DxfSurvey, mut f: impl FnMut(&mut DVec3)) {
    for (_, p) in &mut s.points {
        f(p);
    }
    for l in &mut s.lines {
        for p in &mut l.points {
            f(p);
        }
    }
}

fn looks_geographic(s: &DxfSurvey) -> bool {
    let mut any = false;
    let mut ok = true;
    let mut probe = s.clone();
    each_point(&mut probe, |p| {
        any = true;
        ok &= p.x.abs() <= 180.0 && p.y.abs() <= 90.0;
    });
    any && ok
}

/// Longitude and latitude projected to the UTM zone their middle falls
/// in. Returns the zone's coordinate system.
fn to_utm(s: &mut DxfSurvey) -> String {
    let (mut sum, mut n) = (glam::DVec2::ZERO, 0.0);
    let mut probe = s.clone();
    each_point(&mut probe, |p| {
        sum += p.truncate();
        n += 1.0;
    });
    let mid = sum / f64::max(n, 1.0);
    let (zone, south) = (utm_zone(mid.x), mid.y < 0.0);
    each_point(s, |p| {
        let (e, n) = utm(p.x, p.y, zone, south);
        p.x = e;
        p.y = n;
    });
    format!("EPSG:{}", if south { 32700 } else { 32600 } + zone)
}

fn settle(mut data: MapData, geographic: Option<bool>) -> MapData {
    if geographic.unwrap_or_else(|| looks_geographic(&data.survey)) {
        let crs = to_utm(&mut data.survey);
        data.notes.push(format!(
            "Longitude and latitude were projected to metres in UTM ({crs})."
        ));
        data.crs = Some(crs);
    }
    data
}

fn epsg_is_geographic(code: u32) -> bool {
    matches!(code, 4326 | 4269 | 4258 | 4283 | 4617 | 4979)
}

// ---- LandXML ----

fn numbers(text: &str) -> Vec<f64> {
    text.split(|c: char| c.is_whitespace() || c == ',')
        .filter_map(|t| t.parse().ok())
        .collect()
}

/// A LandXML file's surfaces (their points, break lines and contours)
/// and its survey points. Its triangles are not kept: the ground is
/// built again from the points and break lines.
pub fn landxml(text: &str) -> Result<MapData, String> {
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| format!("this doesn't read as LandXML: {e}"))?;
    let root = doc.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("LandXML") {
        return Err("this XML file is not LandXML".into());
    }
    let named = |n: roxmltree::Node, name: &str| n.tag_name().name() == name;
    let mut out = MapData::default();
    // units
    for u in root.descendants().filter(|n| named(*n, "Units")) {
        for m in u.children().filter(|n| n.is_element()) {
            let unit = m.attribute("linearUnit").unwrap_or("").to_ascii_lowercase();
            out.survey.unit = Some(match unit.as_str() {
                "millimeter" => 0.001,
                "centimeter" => 0.01,
                "kilometer" => 1000.0,
                "foot" => 0.3048,
                "ussurveyfoot" => 1200.0 / 3937.0,
                "inch" => 0.0254,
                _ => 1.0,
            });
        }
    }
    if let Some(cs) = root.descendants().find(|n| named(*n, "CoordinateSystem")) {
        out.crs = cs
            .attribute("epsgCode")
            .filter(|c| !c.trim().is_empty())
            .map(|c| format!("EPSG:{}", c.trim()))
            .or_else(|| cs.attribute("name").map(str::to_string))
            .or_else(|| cs.attribute("desc").map(str::to_string))
            .filter(|c| !c.trim().is_empty());
    }
    // northing, easting, height
    let neh = |v: &[f64]| DVec3::new(v[1], v[0], v[2]);
    for p in root.descendants().filter(|n| named(*n, "CgPoint")) {
        let v = numbers(p.text().unwrap_or(""));
        if v.len() >= 3 {
            let layer = p
                .attribute("code")
                .filter(|c| !c.trim().is_empty())
                .unwrap_or("Points");
            out.survey.points.push((layer.to_string(), neh(&v)));
        }
    }
    let mut thinned = false;
    for (i, s) in root
        .descendants()
        .filter(|n| named(*n, "Surface"))
        .enumerate()
    {
        let name = s
            .attribute("name")
            .filter(|c| !c.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("Surface {}", i + 1));
        let pts: Vec<DVec3> = s
            .descendants()
            .filter(|n| named(*n, "P"))
            .filter_map(|p| {
                let v = numbers(p.text().unwrap_or(""));
                (v.len() >= 3).then(|| neh(&v))
            })
            .collect();
        let step = pts.len().div_ceil(MAX_GRID_POINTS).max(1);
        thinned |= step > 1;
        out.survey
            .points
            .extend(pts.into_iter().step_by(step).map(|p| (name.clone(), p)));
        for b in s.descendants().filter(|n| named(*n, "Breakline")) {
            for l in b.descendants().filter(|n| named(*n, "PntList3D")) {
                let v = numbers(l.text().unwrap_or(""));
                let points: Vec<DVec3> = v.as_chunks::<3>().0.iter().map(|p| neh(p)).collect();
                if points.len() >= 2 {
                    out.survey.lines.push(DxfLine {
                        layer: format!("{name} break lines"),
                        points,
                        closed: false,
                    });
                }
            }
        }
        for c in s.descendants().filter(|n| named(*n, "Contour")) {
            let Some(z) = c
                .attribute("elev")
                .and_then(|e| e.trim().parse::<f64>().ok())
            else {
                continue;
            };
            for l in c.descendants().filter(|n| named(*n, "PntList2D")) {
                let v = numbers(l.text().unwrap_or(""));
                let points: Vec<DVec3> = v
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|p| DVec3::new(p[1], p[0], z))
                    .collect();
                if points.len() >= 2 {
                    out.survey.lines.push(DxfLine {
                        layer: format!("{name} contours"),
                        points,
                        closed: false,
                    });
                }
            }
        }
    }
    if out.survey.points.is_empty() && out.survey.lines.is_empty() {
        return Err("this LandXML file holds no surface and no survey points".into());
    }
    if thinned {
        out.notes.push(format!(
            "A surface of very many points was thinned to about {MAX_GRID_POINTS}."
        ));
    }
    out.notes
        .push("The surface's triangles are not kept: the ground is built again from its points and break lines.".into());
    Ok(out)
}

// ---- height grids ----

/// A grid of heights as points, thinned evenly to [`MAX_GRID_POINTS`].
/// `at(col, row)` is a cell's height, rows counted from the top;
/// `origin` is the middle of the top left cell, `dx` and `dy` a cell's
/// size (`dy` positive: rows run down the map).
fn grid_points(
    cols: usize,
    rows: usize,
    origin: glam::DVec2,
    dx: f64,
    dy: f64,
    at: impl Fn(usize, usize) -> Option<f64>,
) -> (Vec<(String, DVec3)>, usize) {
    let step = (((cols * rows) as f64 / MAX_GRID_POINTS as f64)
        .sqrt()
        .ceil() as usize)
        .max(1);
    let mut out = vec![];
    for r in (0..rows).step_by(step) {
        for c in (0..cols).step_by(step) {
            if let Some(z) = at(c, r).filter(|z| z.is_finite()) {
                out.push((
                    "Heights".to_string(),
                    DVec3::new(origin.x + c as f64 * dx, origin.y - r as f64 * dy, z),
                ));
            }
        }
    }
    (out, step)
}

/// `dx` is a cell's width in the grid's own unit: degrees of longitude
/// at `latitude` when it is given, else metres.
fn grid_note(cols: usize, rows: usize, step: usize, dx: f64, latitude: Option<f64>) -> String {
    let metres = latitude.map_or(dx, |lat| dx * 111_320.0 * lat.to_radians().cos());
    if step > 1 {
        format!(
            "A grid of {cols} by {rows} heights, one in {step} kept each way: one each {:.1} m.",
            metres * step as f64
        )
    } else {
        format!("A grid of {cols} by {rows} heights, one each {metres:.1} m.")
    }
}

/// An Esri ASCII grid (`ncols`, `nrows`, `xllcorner`...).
pub fn ascii_grid(text: &str) -> Result<MapData, String> {
    let mut head = std::collections::HashMap::new();
    let mut rest = text;
    loop {
        let trimmed = rest.trim_start();
        let (line, after) = trimmed.split_once('\n').unwrap_or((trimmed, ""));
        let mut words = line.split_whitespace();
        let (Some(key), Some(value)) = (words.next(), words.next()) else {
            break;
        };
        if !key.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
            break;
        }
        let Ok(v) = value.parse::<f64>() else { break };
        head.insert(key.to_ascii_lowercase(), v);
        rest = after;
    }
    let get = |k: &str| head.get(k).copied();
    let (Some(cols), Some(rows)) = (get("ncols"), get("nrows")) else {
        return Err("this doesn't read as an Esri ASCII grid: no ncols and nrows".into());
    };
    let (cols, rows) = (cols as usize, rows as usize);
    let dx = get("cellsize")
        .or(get("dx"))
        .ok_or("the grid doesn't say its cell size")?;
    let dy = get("cellsize").or(get("dy")).unwrap_or(dx);
    let x0 = get("xllcenter")
        .or(get("xllcorner").map(|x| x + dx / 2.0))
        .unwrap_or(dx / 2.0);
    let y0 = get("yllcenter")
        .or(get("yllcorner").map(|y| y + dy / 2.0))
        .unwrap_or(dy / 2.0);
    let nodata = get("nodata_value").unwrap_or(-9999.0);
    let values: Vec<f64> = rest
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    if cols == 0 || rows == 0 || values.len() < cols * rows {
        return Err(format!(
            "the grid says {cols} by {rows} heights but holds {}",
            values.len()
        ));
    }
    let origin = glam::DVec2::new(x0, y0 + (rows - 1) as f64 * dy);
    let (points, step) = grid_points(cols, rows, origin, dx, dy, |c, r| {
        Some(values[r * cols + c]).filter(|z| (*z - nodata).abs() > 1e-9)
    });
    let mut out = MapData::default();
    out.survey.points = points;
    let geographic = dx < 0.01;
    out.notes
        .push(grid_note(cols, rows, step, dx, geographic.then_some(y0)));
    Ok(settle(out, Some(geographic)))
}

/// A GeoTIFF of heights: one band, placed by its pixel size and tie
/// point.
pub fn geotiff<R: std::io::Read + std::io::Seek>(reader: R) -> Result<MapData, String> {
    use tiff::decoder::{Decoder, DecodingResult, Limits};
    use tiff::tags::Tag;
    let bad = |e: tiff::TiffError| format!("this doesn't read as a GeoTIFF: {e}");
    let mut d = Decoder::new(reader)
        .map_err(bad)?
        .with_limits(Limits::unlimited());
    let (w, h) = d.dimensions().map_err(bad)?;
    let (cols, rows) = (w as usize, h as usize);
    let scale = d.get_tag_f64_vec(Tag::ModelPixelScaleTag).ok();
    let tie = d.get_tag_f64_vec(Tag::ModelTiepointTag).ok();
    let (Some(scale), Some(tie)) = (scale.filter(|s| s.len() >= 2), tie.filter(|t| t.len() >= 6))
    else {
        return Err("this TIFF doesn't say where it is on the map (no pixel size and tie point), so it can't be placed. A turned or skewed GeoTIFF isn't read either.".into());
    };
    let nodata = d
        .get_tag_ascii_string(Tag::GdalNodata)
        .ok()
        .and_then(|s| s.trim().trim_end_matches('\0').trim().parse::<f64>().ok());
    // the coordinate system's EPSG code, from the GeoKey directory
    let keys = d
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .unwrap_or_default();
    let key = |id: u16| {
        keys.as_chunks::<4>()
            .0
            .iter()
            .skip(1)
            .find(|k| k[0] == id && k[1] == 0)
            .map(|k| k[3] as u32)
            .filter(|v| *v != 0 && *v != 32767)
    };
    let (projected, geographic) = (key(3072), key(2048));
    let model = key(1024);
    let values: Vec<f64> = match d.read_image().map_err(bad)? {
        DecodingResult::F32(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::F64(v) => v,
        DecodingResult::I16(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::U16(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::I32(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::U32(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::U8(v) => v.into_iter().map(f64::from).collect(),
        DecodingResult::I8(v) => v.into_iter().map(f64::from).collect(),
        _ => return Err("this TIFF's numbers are of a kind that isn't read as heights".into()),
    };
    if cols == 0 || rows == 0 || values.len() < cols * rows {
        return Err("this TIFF holds fewer heights than its size says".into());
    }
    let bands = values.len() / (cols * rows);
    let (dx, dy) = (scale[0], scale[1]);
    // the tie point: pixel (i, j) is at map (x, y); a pixel's middle is
    // half a pixel on
    let origin = glam::DVec2::new(tie[3] + (0.5 - tie[0]) * dx, tie[4] - (0.5 - tie[1]) * dy);
    let (points, step) = grid_points(cols, rows, origin, dx, dy, |c, r| {
        let z = values[(r * cols + c) * bands];
        Some(z).filter(|z| nodata.is_none_or(|n| (*z - n).abs() > 1e-6) && *z > -1.0e6)
    });
    let mut out = MapData::default();
    out.survey.points = points;
    out.crs = projected.or(geographic).map(|c| format!("EPSG:{c}"));
    let is_geo = model == Some(2)
        || (projected.is_none() && geographic.is_some_and(epsg_is_geographic))
        || (projected.is_none() && geographic.is_none() && dx < 0.01);
    out.notes
        .push(grid_note(cols, rows, step, dx, is_geo.then_some(origin.y)));
    if bands > 1 {
        out.notes
            .push("The picture has several bands; the first is read as heights.".into());
    }
    Ok(settle(out, Some(is_geo)))
}

// ---- GeoJSON ----

const HEIGHT_KEYS: [&str; 9] = [
    "elevation",
    "elev",
    "height",
    "contour",
    "z",
    "level",
    "alt",
    "altitude",
    "hoehe",
];
const LAYER_KEYS: [&str; 9] = [
    "layer", "class", "fclass", "type", "highway", "building", "landuse", "natural", "name",
];

fn prop_height(props: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<f64> {
    let props = props?;
    HEIGHT_KEYS.iter().find_map(|k| {
        props
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(k))
            .and_then(|(_, v)| v.as_f64().or_else(|| v.as_str()?.trim().parse().ok()))
    })
}

fn prop_layer(props: Option<&serde_json::Map<String, serde_json::Value>>) -> Option<String> {
    let props = props?;
    LAYER_KEYS.iter().find_map(|k| {
        let (name, v) = props
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(k))?;
        let v = v.as_str()?.trim();
        (!v.is_empty()).then(|| {
            if matches!(
                name.to_ascii_lowercase().as_str(),
                "highway" | "building" | "landuse" | "natural"
            ) {
                format!("{name}: {v}")
            } else {
                v.to_string()
            }
        })
    })
}

fn coord(v: &serde_json::Value, height: Option<f64>) -> Option<DVec3> {
    let a = v.as_array()?;
    let (x, y) = (a.first()?.as_f64()?, a.get(1)?.as_f64()?);
    let z = a
        .get(2)
        .and_then(|z| z.as_f64())
        .filter(|z| z.abs() > 1e-9)
        .or(height)
        .unwrap_or(0.0);
    Some(DVec3::new(x, y, z))
}

fn line_of(v: &serde_json::Value, height: Option<f64>) -> Vec<DVec3> {
    v.as_array()
        .map(|a| a.iter().filter_map(|c| coord(c, height)).collect())
        .unwrap_or_default()
}

fn geometry(
    g: &serde_json::Value,
    props: Option<&serde_json::Map<String, serde_json::Value>>,
    out: &mut DxfSurvey,
) {
    let kind = g.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let height = prop_height(props);
    let layer = prop_layer(props);
    let named = |fallback: &str| layer.clone().unwrap_or_else(|| fallback.to_string());
    let coords = g.get("coordinates").unwrap_or(&serde_json::Value::Null);
    let each = |v: &serde_json::Value| v.as_array().cloned().unwrap_or_default();
    let line = |points: Vec<DVec3>, closed: bool, fallback: &str, out: &mut DxfSurvey| {
        let mut points = points;
        if closed && points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        if points.len() >= 2 {
            out.lines.push(DxfLine {
                layer: named(fallback),
                points,
                closed,
            });
        }
    };
    match kind {
        "Point" => {
            if let Some(p) = coord(coords, height).filter(|p| p.z.abs() > 1e-9) {
                out.points.push((named("Points"), p));
            }
        }
        "MultiPoint" => {
            for c in each(coords) {
                if let Some(p) = coord(&c, height).filter(|p| p.z.abs() > 1e-9) {
                    out.points.push((named("Points"), p));
                }
            }
        }
        "LineString" => line(line_of(coords, height), false, "Lines", out),
        "MultiLineString" => {
            for l in each(coords) {
                line(line_of(&l, height), false, "Lines", out);
            }
        }
        "Polygon" => {
            for ring in each(coords) {
                line(line_of(&ring, height), true, "Outlines", out);
            }
        }
        "MultiPolygon" => {
            for poly in each(coords) {
                for ring in each(&poly) {
                    line(line_of(&ring, height), true, "Outlines", out);
                }
            }
        }
        "GeometryCollection" => {
            for g in each(g.get("geometries").unwrap_or(&serde_json::Value::Null)) {
                geometry(&g, props, out);
            }
        }
        _ => {}
    }
}

/// A GeoJSON file: points with a height, lines and outlines. A height
/// comes from a third coordinate or from a property named for one
/// (`elevation`, `elev`, `height`, `contour`...).
pub fn geojson(text: &str) -> Result<MapData, String> {
    let v: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("this doesn't read as GeoJSON: {e}"))?;
    let mut out = MapData::default();
    let feature = |f: &serde_json::Value, out: &mut DxfSurvey| {
        if let Some(g) = f.get("geometry").filter(|g| !g.is_null()) {
            geometry(g, f.get("properties").and_then(|p| p.as_object()), out);
        }
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("FeatureCollection") => {
            for f in v
                .get("features")
                .and_then(|f| f.as_array())
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                feature(f, &mut out.survey);
            }
        }
        Some("Feature") => feature(&v, &mut out.survey),
        Some(_) => geometry(&v, None, &mut out.survey),
        None => return Err("this JSON file is not GeoJSON".into()),
    }
    if out.survey.points.is_empty() && out.survey.lines.is_empty() {
        return Err("this GeoJSON file holds no points with a height and no lines".into());
    }
    // an old-style named coordinate system; without one it is WGS 84
    let named = v
        .pointer("/crs/properties/name")
        .and_then(|n| n.as_str())
        .and_then(|n| n.rsplit(':').next()?.parse::<u32>().ok());
    let geographic = match named {
        Some(code) if !epsg_is_geographic(code) => {
            out.crs = Some(format!("EPSG:{code}"));
            Some(false)
        }
        Some(_) => Some(true),
        None => None,
    };
    Ok(settle(out, geographic))
}

// ---- shapefiles ----

fn le_i32(b: &[u8], at: usize) -> Option<i32> {
    Some(i32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

fn le_f64(b: &[u8], at: usize) -> Option<f64> {
    Some(f64::from_le_bytes(b.get(at..at + 8)?.try_into().ok()?))
}

/// The heights a shapefile's table gives its records, from the first
/// column named for one.
fn dbf_heights(dbf: &[u8]) -> Option<Vec<Option<f64>>> {
    let count = u32::from_le_bytes(dbf.get(4..8)?.try_into().ok()?) as usize;
    let head = u16::from_le_bytes(dbf.get(8..10)?.try_into().ok()?) as usize;
    let size = u16::from_le_bytes(dbf.get(10..12)?.try_into().ok()?) as usize;
    let (mut at, mut offset, mut found) = (32, 1, None);
    while at + 32 <= dbf.len() && dbf[at] != 0x0D {
        let name: String = dbf[at..at + 11]
            .iter()
            .take_while(|c| **c != 0)
            .map(|c| *c as char)
            .collect();
        let len = dbf[at + 16] as usize;
        if found.is_none()
            && HEIGHT_KEYS
                .iter()
                .any(|k| name.trim().eq_ignore_ascii_case(k))
        {
            found = Some((offset, len));
        }
        offset += len;
        at += 32;
    }
    let (offset, len) = found?;
    Some(
        (0..count)
            .map(|i| {
                let start = head + i * size + offset;
                let cell = dbf.get(start..start + len)?;
                String::from_utf8_lossy(cell).trim().parse().ok()
            })
            .collect(),
    )
}

/// A shapefile's points, lines and outlines, on a layer named `layer`.
/// Heights come from its Z values, or from a column of its table
/// (`dbf`) named for one. `prj` is its coordinate system as it writes it.
pub fn shapefile(
    shp: &[u8],
    dbf: Option<&[u8]>,
    prj: Option<&str>,
    layer: &str,
) -> Result<MapData, String> {
    if shp.len() < 100 || shp[..4] != [0, 0, 0x27, 0x0A] {
        return Err("this doesn't read as a shapefile".into());
    }
    let heights = dbf.and_then(dbf_heights).unwrap_or_default();
    let mut out = MapData::default();
    let (mut at, mut record) = (100, 0);
    while at + 12 <= shp.len() {
        let words = u32::from_be_bytes(shp[at + 4..at + 8].try_into().unwrap()) as usize;
        let body = at + 8;
        let end = (body + words * 2).min(shp.len());
        let table_z = heights.get(record).copied().flatten();
        record += 1;
        at = end;
        let b = &shp[body..end];
        let Some(kind) = le_i32(b, 0) else { break };
        let has_z = matches!(kind, 11 | 13 | 15 | 18);
        match kind % 10 {
            1 => {
                let (Some(x), Some(y)) = (le_f64(b, 4), le_f64(b, 12)) else {
                    continue;
                };
                let z = has_z
                    .then(|| le_f64(b, 20))
                    .flatten()
                    .filter(|z| z.abs() > 1e-9 && *z > -1.0e30)
                    .or(table_z);
                if let Some(z) = z {
                    out.survey
                        .points
                        .push((layer.to_string(), DVec3::new(x, y, z)));
                }
            }
            3 | 5 => {
                let (Some(parts), Some(n)) = (le_i32(b, 36), le_i32(b, 40)) else {
                    continue;
                };
                let (parts, n) = (parts.max(0) as usize, n.max(0) as usize);
                let xy = 44 + parts * 4;
                let zs = xy + n * 16 + 16;
                if b.len() < xy + n * 16 {
                    continue;
                }
                let starts: Vec<usize> = (0..parts)
                    .filter_map(|p| le_i32(b, 44 + p * 4).map(|s| s.max(0) as usize))
                    .collect();
                for (p, start) in starts.iter().enumerate() {
                    let stop = starts.get(p + 1).copied().unwrap_or(n).min(n);
                    let mut points: Vec<DVec3> = (*start..stop)
                        .filter_map(|i| {
                            let (x, y) = (le_f64(b, xy + i * 16)?, le_f64(b, xy + i * 16 + 8)?);
                            let z = has_z
                                .then(|| le_f64(b, zs + i * 8))
                                .flatten()
                                .filter(|z| *z > -1.0e30)
                                .unwrap_or(0.0);
                            Some(DVec3::new(x, y, z))
                        })
                        .collect();
                    if points.iter().all(|q| q.z.abs() < 1e-9) {
                        if let Some(z) = table_z {
                            for q in &mut points {
                                q.z = z;
                            }
                        }
                    }
                    let closed = kind % 10 == 5;
                    if closed && points.len() > 1 && points.first() == points.last() {
                        points.pop();
                    }
                    if points.len() >= 2 {
                        out.survey.lines.push(DxfLine {
                            layer: layer.to_string(),
                            points,
                            closed,
                        });
                    }
                }
            }
            8 => {
                let Some(n) = le_i32(b, 36) else { continue };
                let n = n.max(0) as usize;
                let zs = 40 + n * 16 + 16;
                for i in 0..n {
                    let (Some(x), Some(y)) = (le_f64(b, 40 + i * 16), le_f64(b, 48 + i * 16))
                    else {
                        continue;
                    };
                    let z = has_z
                        .then(|| le_f64(b, zs + i * 8))
                        .flatten()
                        .filter(|z| z.abs() > 1e-9 && *z > -1.0e30)
                        .or(table_z);
                    if let Some(z) = z {
                        out.survey
                            .points
                            .push((layer.to_string(), DVec3::new(x, y, z)));
                    }
                }
            }
            _ => {}
        }
    }
    if out.survey.points.is_empty() && out.survey.lines.is_empty() {
        return Err("this shapefile holds no points with a height and no lines".into());
    }
    let mut geographic = None;
    if let Some(prj) = prj.map(str::trim).filter(|p| !p.is_empty()) {
        let upper = prj.to_ascii_uppercase();
        geographic = Some(upper.starts_with("GEOGCS") || upper.starts_with("GEOGCRS"));
        if geographic == Some(false) {
            // its EPSG code if it carries one at the end, else its name
            let code = upper
                .rfind("AUTHORITY[\"EPSG\",\"")
                .and_then(|i| upper[i + 18..].split('"').next()?.parse::<u32>().ok());
            out.crs = code
                .map(|c| format!("EPSG:{c}"))
                .or_else(|| prj.split('"').nth(1).map(str::to_string));
        }
    }
    Ok(settle(out, geographic))
}

// ---- OpenStreetMap ----

/// The tags an OpenStreetMap way is sorted by, in order: the first it
/// carries names its layer ("building", "highway: residential").
const OSM_KINDS: [&str; 12] = [
    "building", "highway", "railway", "waterway", "natural", "landuse", "leisure", "amenity",
    "barrier", "boundary", "man_made", "place",
];

/// An OpenStreetMap export (`.osm`, as the website's Export button gives
/// it): its ways as lines by kind, buildings and areas closed. A way
/// tagged with its height (`ele`) is a contour.
pub fn osm(text: &str) -> Result<MapData, String> {
    let doc = roxmltree::Document::parse(text)
        .map_err(|e| format!("this doesn't read as an OpenStreetMap file: {e}"))?;
    let root = doc.root_element();
    if root.tag_name().name() != "osm" {
        return Err("this XML file is not an OpenStreetMap export".into());
    }
    let mut nodes = std::collections::HashMap::new();
    for n in root.children().filter(|n| n.has_tag_name("node")) {
        let at = |k| n.attribute(k).and_then(|v: &str| v.parse::<f64>().ok());
        if let (Some(id), Some(lat), Some(lon)) = (n.attribute("id"), at("lat"), at("lon")) {
            nodes.insert(id, (lon, lat));
        }
    }
    let mut out = MapData::default();
    for w in root.children().filter(|n| n.has_tag_name("way")) {
        let tag = |k: &str| {
            w.children()
                .filter(|t| t.has_tag_name("tag"))
                .find(|t| t.attribute("k") == Some(k))
                .and_then(|t| t.attribute("v"))
        };
        let Some(layer) = OSM_KINDS.iter().find_map(|k| {
            tag(k).map(|v| match (*k, v) {
                ("building", _) => "building".to_string(),
                (k, v) => format!("{k}: {v}"),
            })
        }) else {
            continue;
        };
        let height = tag("ele").and_then(|v| v.trim().parse::<f64>().ok());
        let refs: Vec<&str> = w
            .children()
            .filter(|n| n.has_tag_name("nd"))
            .filter_map(|n| n.attribute("ref"))
            .collect();
        let closed = refs.len() > 2 && refs.first() == refs.last();
        let mut points: Vec<DVec3> = refs
            .iter()
            .filter_map(|r| nodes.get(r))
            .map(|(x, y)| DVec3::new(*x, *y, height.unwrap_or(0.0)))
            .collect();
        if closed {
            points.pop();
        }
        if points.len() >= 2 {
            out.survey.lines.push(DxfLine {
                layer,
                points,
                closed,
            });
        }
    }
    if out.survey.lines.is_empty() {
        return Err("this OpenStreetMap file holds no buildings, roads or areas".into());
    }
    Ok(settle(out, Some(true)))
}

// ---- pictures on the map ----

/// A picture placed on the map: an aerial photo or a scanned map with its
/// place (GeoTIFF).
#[derive(Clone, Debug, PartialEq)]
pub struct MapPicture {
    pub width: usize,
    pub height: usize,
    /// Its pixels, rows from the top.
    pub rgba: Vec<u8>,
    /// Its lower left and upper right corners on the map, metres.
    pub lo: glam::DVec2,
    pub hi: glam::DVec2,
    pub crs: Option<String>,
}

/// Whether a TIFF holds a picture (colours) rather than heights.
pub fn is_picture(path: &Path) -> bool {
    use tiff::ColorType;
    std::fs::File::open(path)
        .ok()
        .and_then(|f| tiff::decoder::Decoder::new(std::io::BufReader::new(f)).ok())
        .and_then(|mut d| d.colortype().ok())
        .is_some_and(|c| {
            matches!(
                c,
                ColorType::RGB(8) | ColorType::RGBA(8) | ColorType::Palette(8)
            ) || matches!(
                c,
                ColorType::Multiband {
                    bit_depth: 8,
                    num_samples: 3..
                }
            )
        })
}

/// A GeoTIFF picture with its place. One in longitude and latitude has
/// its corners projected to its UTM zone: over a site the picture's
/// slight bend is less than a pixel.
pub fn map_picture<R: std::io::Read + std::io::Seek>(reader: R) -> Result<MapPicture, String> {
    use tiff::decoder::{Decoder, DecodingResult, Limits};
    use tiff::tags::Tag;
    use tiff::ColorType;
    let bad = |e: tiff::TiffError| format!("this doesn't read as a GeoTIFF picture: {e}");
    let mut d = Decoder::new(reader)
        .map_err(bad)?
        .with_limits(Limits::unlimited());
    let (w, h) = d.dimensions().map_err(bad)?;
    let (w, h) = (w as usize, h as usize);
    let scale = d.get_tag_f64_vec(Tag::ModelPixelScaleTag).ok();
    let tie = d.get_tag_f64_vec(Tag::ModelTiepointTag).ok();
    let (Some(scale), Some(tie)) = (scale.filter(|s| s.len() >= 2), tie.filter(|t| t.len() >= 6))
    else {
        return Err("this picture doesn't say where it is on the map (no pixel size and tie point). Add it as a plain picture and place it with Align.".into());
    };
    let keys = d
        .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
        .unwrap_or_default();
    let key = |id: u16| {
        keys.as_chunks::<4>()
            .0
            .iter()
            .skip(1)
            .find(|k| k[0] == id && k[1] == 0)
            .map(|k| k[3] as u32)
            .filter(|v| *v != 0 && *v != 32767)
    };
    let (projected, geographic, model) = (key(3072), key(2048), key(1024));
    let colour = d.colortype().map_err(bad)?;
    let palette = match colour {
        ColorType::Palette(_) => d.get_tag_u16_vec(Tag::ColorMap).ok(),
        _ => None,
    };
    let DecodingResult::U8(px) = d.read_image().map_err(bad)? else {
        return Err("only pictures of 8 bits a colour are read".into());
    };
    let n = w * h;
    let bands = px.len() / n.max(1);
    let mut rgba = Vec::with_capacity(n * 4);
    for i in 0..n {
        let p = &px[i * bands..(i + 1) * bands];
        let c = match (&palette, bands) {
            (Some(map), _) if map.len() >= 768 => {
                let k = p[0] as usize;
                let at = |j: usize| (map[j * 256 + k] >> 8) as u8;
                [at(0), at(1), at(2), 255]
            }
            (_, 1 | 2) => [p[0], p[0], p[0], 255],
            (_, 3) => [p[0], p[1], p[2], 255],
            _ => [p[0], p[1], p[2], p[3]],
        };
        rgba.extend(c);
    }
    let (dx, dy) = (scale[0], scale[1]);
    let left = tie[3] - tie[0] * dx;
    let top = tie[4] + tie[1] * dy;
    let (mut lo, mut hi) = (
        glam::DVec2::new(left, top - h as f64 * dy),
        glam::DVec2::new(left + w as f64 * dx, top),
    );
    let mut crs = projected.or(geographic).map(|c| format!("EPSG:{c}"));
    let is_geo = model == Some(2)
        || (projected.is_none() && geographic.is_some_and(epsg_is_geographic))
        || (projected.is_none() && geographic.is_none() && dx < 0.01);
    if is_geo {
        let mid = (lo + hi) / 2.0;
        let (zone, south) = (utm_zone(mid.x), mid.y < 0.0);
        let (x0, y0) = utm(lo.x, lo.y, zone, south);
        let (x1, y1) = utm(hi.x, hi.y, zone, south);
        lo = glam::DVec2::new(x0, y0);
        hi = glam::DVec2::new(x1, y1);
        crs = Some(format!("EPSG:{}", if south { 32700 } else { 32600 } + zone));
    }
    Ok(MapPicture {
        width: w,
        height: h,
        rgba,
        lo,
        hi,
        crs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn longitude_and_latitude_land_in_their_utm_zone() {
        // checked against the Krueger series: near the CN Tower, Toronto
        assert_eq!(utm_zone(-79.3871), 17);
        let (e, n) = utm(-79.3871, 43.6426, 17, false);
        assert!((e - 630_087.375).abs() < 0.01, "{e}");
        assert!((n - 4_833_442.312).abs() < 0.01, "{n}");
        // and near the Sydney Opera House, south of the equator
        assert_eq!(utm_zone(151.2153), 56);
        let (e, n) = utm(151.2153, -33.8568, 56, true);
        assert!((e - 334_900.570).abs() < 0.01, "{e}");
        assert!((n - 6_252_288.753).abs() < 0.01, "{n}");
    }

    #[test]
    fn a_landxml_surface_gives_points_break_lines_and_contours() {
        let xml = r#"<?xml version="1.0"?>
<LandXML xmlns="http://www.landxml.org/schema/LandXML-1.2" version="1.2">
 <Units><Metric linearUnit="meter" areaUnit="squareMeter"/></Units>
 <CoordinateSystem epsgCode="2952" name="NAD83(CSRS) / MTM zone 10"/>
 <CgPoints><CgPoint name="1" code="TREE">4830010.0 310020.0 101.5</CgPoint></CgPoints>
 <Surfaces><Surface name="EG">
  <SourceData>
   <Breaklines><Breakline brkType="standard"><PntList3D>4830000 310000 100 4830000 310050 101 4830040 310050 103</PntList3D></Breakline></Breaklines>
   <Contours><Contour elev="102.0"><PntList2D>4830020 310000 4830022 310050</PntList2D></Contour></Contours>
  </SourceData>
  <Definition surfType="TIN">
   <Pnts><P id="1">4830000 310000 100</P><P id="2">4830000 310050 101</P><P id="3">4830040 310050 103</P><P id="4">4830040 310000 102</P></Pnts>
   <Faces><F>1 2 3</F><F>1 3 4</F></Faces>
  </Definition>
 </Surface></Surfaces>
</LandXML>"#;
        let d = landxml(xml).unwrap();
        assert_eq!(d.crs.as_deref(), Some("EPSG:2952"));
        assert_eq!(d.survey.unit, Some(1.0));
        assert_eq!(d.survey.points.len(), 5);
        // northing then easting in the file: x is the easting
        assert_eq!(
            d.survey.points[0],
            ("TREE".into(), DVec3::new(310020.0, 4830010.0, 101.5))
        );
        let layers = d.survey.layers();
        let of = |n: &str| layers.iter().find(|l| l.name == n).unwrap().clone();
        assert_eq!(of("EG").points, 4);
        assert_eq!(of("EG break lines").breaks, 1);
        assert_eq!(of("EG contours").contours, 1);
        assert!(landxml("<a/>").is_err());
    }

    #[test]
    fn an_ascii_grid_is_read_from_its_top_row_and_thinned() {
        let text = "ncols 3\nnrows 2\nxllcorner 1000\nyllcorner 2000\ncellsize 10\nNODATA_value -9999\n1 2 3\n4 -9999 6\n";
        let d = ascii_grid(text).unwrap();
        let p: Vec<DVec3> = d.survey.points.iter().map(|p| p.1).collect();
        assert_eq!(p.len(), 5);
        // the first value is the top left cell's middle
        assert_eq!(p[0], DVec3::new(1005.0, 2015.0, 1.0));
        assert_eq!(p[4], DVec3::new(1025.0, 2005.0, 6.0));
        // a large grid keeps every so many
        let (cols, rows) = (400, 300);
        let mut big = format!("ncols {cols}\nnrows {rows}\nxllcorner 0\nyllcorner 0\ncellsize 1\n");
        for r in 0..rows {
            for c in 0..cols {
                big.push_str(&format!("{} ", r + c));
            }
            big.push('\n');
        }
        let d = ascii_grid(&big).unwrap();
        assert!(d.survey.points.len() <= MAX_GRID_POINTS && d.survey.points.len() > 5000);
        assert!(d.notes[0].contains("one in 4 kept"), "{}", d.notes[0]);
    }

    #[test]
    fn a_geotiff_of_heights_is_placed_by_its_tie_point() {
        use tiff::encoder::{colortype, TiffEncoder};
        use tiff::tags::Tag;
        let mut bytes = std::io::Cursor::new(vec![]);
        {
            let mut enc = TiffEncoder::new(&mut bytes).unwrap();
            let mut img = enc.new_image::<colortype::Gray32Float>(3, 2).unwrap();
            img.encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[2.0f64, 2.0, 0.0][..])
                .unwrap();
            img.encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0f64, 0.0, 0.0, 500_000.0, 4_800_000.0, 0.0][..],
                )
                .unwrap();
            img.encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[1u16, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32617][..],
                )
                .unwrap();
            img.write_data(&[10.0f32, 11.0, 12.0, 20.0, 21.0, 22.0])
                .unwrap();
        }
        bytes.set_position(0);
        let d = geotiff(bytes).unwrap();
        assert_eq!(d.crs.as_deref(), Some("EPSG:32617"));
        let p: Vec<DVec3> = d.survey.points.iter().map(|p| p.1).collect();
        assert_eq!(p.len(), 6);
        assert_eq!(p[0], DVec3::new(500_001.0, 4_799_999.0, 10.0));
        assert_eq!(p[5], DVec3::new(500_005.0, 4_799_997.0, 22.0));
    }

    #[test]
    fn geojson_contours_come_in_metres_with_their_heights() {
        let text = r#"{"type":"FeatureCollection","features":[
          {"type":"Feature","properties":{"ELEV":"112.5"},"geometry":{"type":"LineString","coordinates":[[-79.3871,43.6426],[-79.3861,43.6427]]}},
          {"type":"Feature","properties":{"highway":"residential"},"geometry":{"type":"LineString","coordinates":[[-79.3871,43.6420],[-79.3861,43.6421]]}},
          {"type":"Feature","properties":{"building":"yes"},"geometry":{"type":"Polygon","coordinates":[[[-79.387,43.642],[-79.3869,43.642],[-79.3869,43.6421],[-79.387,43.642]]]}},
          {"type":"Feature","properties":{},"geometry":{"type":"Point","coordinates":[-79.3865,43.6424,110.2]}}
        ]}"#;
        let d = geojson(text).unwrap();
        assert_eq!(d.crs.as_deref(), Some("EPSG:32617"));
        assert!(d.notes.iter().any(|n| n.contains("UTM")));
        let c = &d.survey.lines[0];
        assert!(c.is_contour() && (c.points[0].z - 112.5).abs() < 1e-9);
        assert!((c.points[0].x - 630_087.4).abs() < 0.1);
        // about 80 m along
        assert!((c.points[0].distance(c.points[1]) - 81.0).abs() < 2.0);
        assert_eq!(d.survey.lines[1].layer, "highway: residential");
        assert!(!d.survey.lines[1].is_contour() && !d.survey.lines[1].is_break());
        assert!(d.survey.lines[2].closed && d.survey.lines[2].points.len() == 3);
        assert_eq!(d.survey.points.len(), 1);
        // a projected system named the old way is left as it is
        let text = r#"{"type":"Feature","crs":{"type":"name","properties":{"name":"urn:ogc:def:crs:EPSG::2952"}},
          "properties":{"elevation":95},"geometry":{"type":"LineString","coordinates":[[310000,4830000],[310010,4830000]]}}"#;
        let d = geojson(text).unwrap();
        assert_eq!(d.crs.as_deref(), Some("EPSG:2952"));
        assert_eq!(
            d.survey.lines[0].points[1],
            DVec3::new(310010.0, 4830000.0, 95.0)
        );
    }

    /// A shapefile of one polyline record, with or without Z.
    fn shp_line(points: &[DVec3], z: bool) -> Vec<u8> {
        let mut body = vec![];
        body.extend((if z { 13i32 } else { 3 }).to_le_bytes());
        body.extend([0u8; 32]);
        body.extend(1i32.to_le_bytes());
        body.extend((points.len() as i32).to_le_bytes());
        body.extend(0i32.to_le_bytes());
        for p in points {
            body.extend(p.x.to_le_bytes());
            body.extend(p.y.to_le_bytes());
        }
        if z {
            body.extend([0u8; 16]);
            for p in points {
                body.extend(p.z.to_le_bytes());
            }
        }
        let mut out = vec![0u8; 100];
        out[..4].copy_from_slice(&9994i32.to_be_bytes());
        out[32..36].copy_from_slice(&(if z { 13i32 } else { 3 }).to_le_bytes());
        out.extend(1i32.to_be_bytes());
        out.extend(((body.len() / 2) as i32).to_be_bytes());
        out.extend(body);
        out
    }

    /// A table of one numeric column and one record.
    fn dbf(name: &str, value: &str) -> Vec<u8> {
        let len = 10usize;
        let mut out = vec![0u8; 32];
        out[0] = 3;
        out[4..8].copy_from_slice(&1u32.to_le_bytes());
        out[8..10].copy_from_slice(&(32u16 + 32 + 1).to_le_bytes());
        out[10..12].copy_from_slice(&(1 + len as u16).to_le_bytes());
        let mut field = [0u8; 32];
        field[..name.len()].copy_from_slice(name.as_bytes());
        field[11] = b'N';
        field[16] = len as u8;
        out.extend(field);
        out.push(0x0D);
        out.push(b' ');
        out.extend(format!("{value:>len$}").as_bytes());
        out
    }

    #[test]
    fn a_shapefile_takes_heights_from_its_z_or_its_table() {
        let pts = [
            DVec3::new(310000.0, 4830000.0, 101.0),
            DVec3::new(310020.0, 4830000.0, 103.0),
        ];
        let prj = r#"PROJCS["NAD83_CSRS_MTM_zone_10",GEOGCS["GCS"],AUTHORITY["EPSG","2952"]]"#;
        let d = shapefile(&shp_line(&pts, true), None, Some(prj), "kerb").unwrap();
        assert_eq!(d.crs.as_deref(), Some("EPSG:2952"));
        assert!(d.survey.lines[0].is_break());
        assert_eq!(d.survey.lines[0].points[1], pts[1]);
        assert_eq!(d.survey.lines[0].layer, "kerb");
        // flat lines with a height column are contours
        let d = shapefile(
            &shp_line(&pts, false),
            Some(&dbf("ELEV", "98.5")),
            None,
            "contours",
        )
        .unwrap();
        assert!(d.survey.lines[0].is_contour());
        assert_eq!(d.survey.lines[0].points[0].z, 98.5);
        // and without one they carry none
        let d = shapefile(&shp_line(&pts, false), None, None, "roads").unwrap();
        assert!(!d.survey.lines[0].is_contour() && !d.survey.lines[0].is_break());
        assert!(shapefile(b"not a shapefile", None, None, "x").is_err());
    }

    #[test]
    fn an_openstreetmap_export_comes_in_by_kind() {
        let text = r#"<?xml version="1.0"?><osm version="0.6">
          <node id="1" lat="43.6426" lon="-79.3871"/><node id="2" lat="43.6426" lon="-79.3866"/>
          <node id="3" lat="43.6429" lon="-79.3866"/><node id="4" lat="43.6429" lon="-79.3871"/>
          <node id="5" lat="43.6420" lon="-79.3875"/><node id="6" lat="43.6420" lon="-79.3860"/>
          <way id="10"><nd ref="1"/><nd ref="2"/><nd ref="3"/><nd ref="4"/><nd ref="1"/><tag k="building" v="house"/></way>
          <way id="11"><nd ref="5"/><nd ref="6"/><tag k="highway" v="residential"/><tag k="name" v="Front St"/></way>
          <way id="12"><nd ref="5"/><nd ref="6"/><tag k="note" v="untagged kind"/></way>
        </osm>"#;
        let d = osm(text).unwrap();
        assert_eq!(d.crs.as_deref(), Some("EPSG:32617"));
        assert_eq!(d.survey.lines.len(), 2);
        let b = &d.survey.lines[0];
        assert_eq!(b.layer, "building");
        assert!(b.closed && b.points.len() == 4);
        // about 40 m along the street side
        assert!((b.points[0].distance(b.points[1]) - 40.3).abs() < 1.0);
        assert_eq!(d.survey.lines[1].layer, "highway: residential");
        assert!(osm("<osm/>").is_err());
    }

    #[test]
    fn a_geotiff_picture_is_placed_and_coloured() {
        use tiff::encoder::{colortype, TiffEncoder};
        use tiff::tags::Tag;
        let mut bytes = std::io::Cursor::new(vec![]);
        {
            let mut enc = TiffEncoder::new(&mut bytes).unwrap();
            let mut img = enc.new_image::<colortype::RGB8>(2, 2).unwrap();
            img.encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[0.5f64, 0.5, 0.0][..])
                .unwrap();
            img.encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0f64, 0.0, 0.0, 630_000.0, 4_833_500.0, 0.0][..],
                )
                .unwrap();
            img.encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[1u16, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32617][..],
                )
                .unwrap();
            img.write_data(&[255u8, 0, 0, 0, 255, 0, 0, 0, 255, 9, 9, 9])
                .unwrap();
        }
        bytes.set_position(0);
        let p = map_picture(bytes).unwrap();
        assert_eq!((p.width, p.height), (2, 2));
        assert_eq!(&p.rgba[..8], &[255, 0, 0, 255, 0, 255, 0, 255]);
        assert_eq!(p.lo, glam::DVec2::new(630_000.0, 4_833_499.0));
        assert_eq!(p.hi, glam::DVec2::new(630_001.0, 4_833_500.0));
        assert_eq!(p.crs.as_deref(), Some("EPSG:32617"));
    }
}

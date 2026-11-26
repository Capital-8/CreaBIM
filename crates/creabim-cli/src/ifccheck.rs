//! `creabim ifc-check`: the IFC bench (stage 1 of the exchange roadmap).
//!
//! For each file: import it, save the model as a `.creabim` project, open
//! that again, export IFC4, and compare the source with what came out
//! (see [`creabim_ifc::census`]). It also checks that two exports of the
//! same model agree, counts what one edit changes in the file, compares
//! each element's shape after the export is imported again, and times
//! each step. It measures and reports only: nothing is repaired.

use creabim_core::{Document, ElementKind, Model, Transaction};
use creabim_ifc::census::{Census, Measure};
use creabim_ifc::step_read::parse;
use creabim_ifc::{export_ifc4, import_ifc, ExportOptions};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

/// One file's results.
pub struct Run {
    pub file: String,
    pub bytes: usize,
    pub schema: String,
    pub source_objects: usize,
    pub imported: usize,
    pub meshes: usize,
    pub warnings: usize,
    pub measures: Vec<Measure>,
    /// Elements that differ after the `.creabim` save and open.
    pub store_changed: usize,
    pub store_missing: usize,
    /// Lines that differ between two exports of the same model.
    pub export_noise: usize,
    /// ifcmerge's rules: source records whose `#id` comes back holding
    /// the same class, and source records written back byte for byte, of
    /// all the source's records.
    pub ids_kept: usize,
    pub records_same: usize,
    pub records: usize,
    /// The one-edit test: the edited element, lines in the file, lines
    /// changed by the edit.
    pub edit: Option<(String, usize, usize)>,
    /// Geometry after export and import again: elements compared, gone,
    /// moved or resized more than 1 mm, and the largest difference (m).
    pub geometry: (usize, usize, usize, f64),
    /// The elements furthest off, "IfcWall 'name' (GlobalId) 12.0 mm".
    pub geometry_worst: Vec<String>,
    /// CreaBIM's geometry against IfcOpenShell's, the engine Bonsai draws
    /// with, when `<file>.ref.json` lies beside the file (made by
    /// `tools/ifc-bench/reference.py`).
    pub reference: Option<RefDiff>,
    /// Seconds: import, save, open, export.
    pub times: [f64; 4],
    pub classes_in: Vec<(String, usize)>,
    pub classes_model: Vec<(String, usize, usize)>,
}

/// Each element's body as CreaBIM imports it against the same element as
/// IfcOpenShell meshes it: the same within [`REF_BOX`] and [`REF_VOLUME`],
/// or how it differs.
#[derive(Debug, Clone, Default)]
pub struct RefDiff {
    pub engine: String,
    /// Elements with a body in the reference.
    pub reference: usize,
    pub same: usize,
    /// The box differs by more than [`REF_BOX`] (moved, cut or grown).
    pub box_off: usize,
    /// The box is the same but the volume differs by more than
    /// [`REF_VOLUME`] (a cut inside the box: a mitre, a notch, a hole).
    pub volume_off: usize,
    /// In the reference with a body; no body in CreaBIM.
    pub missing: usize,
    /// A body in CreaBIM and none in the reference.
    pub extra: usize,
    /// Reference bodies that are not building bodies (openings, which
    /// only cut; annotations; reference lines and planes), left out.
    pub not_bodies: usize,
    /// By class: same, box off, volume off, missing.
    pub by_class: Vec<(String, [usize; 4])>,
    /// The furthest off, worst first.
    pub worst: Vec<String>,
}

/// Reference classes that are not building bodies.
const NOT_BODIES: [&str; 3] = ["IfcOpeningElement", "IfcAnnotation", "IfcVirtualElement"];

/// How far a box may differ from the reference's and still be the same (m).
pub const REF_BOX: f64 = 0.01;
/// How far a volume may differ, as a share of the reference's.
pub const REF_VOLUME: f64 = 0.02;

/// Compare `model`'s bodies with the reference file beside `path`, if
/// there is one.
fn reference_diff(path: &str, model: &Model) -> Option<RefDiff> {
    use creabim_geom::scene::{build_scene_for_exchange, element_meshes};
    let p = Path::new(path).with_extension("ref.json");
    let text = std::fs::read_to_string(&p).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let rows = json.get("elements")?.as_object()?;
    let ours: HashMap<String, (creabim_core::ElementId, creabim_geom::TriMesh)> =
        element_meshes(&build_scene_for_exchange(model))
            .into_iter()
            .map(|(id, m)| (id.ifc_guid(), (id, m)))
            .collect();
    let v3 = |v: &serde_json::Value| -> Option<[f64; 3]> {
        let a = v.as_array()?;
        Some([
            a.first()?.as_f64()?,
            a.get(1)?.as_f64()?,
            a.get(2)?.as_f64()?,
        ])
    };
    let mut d = RefDiff {
        engine: json
            .get("engine")
            .and_then(|e| e.as_str())
            .unwrap_or("?")
            .into(),
        reference: rows.len(),
        ..Default::default()
    };
    // (how far off, line)
    let mut offs: Vec<(f64, String)> = vec![];
    let mut by: std::collections::BTreeMap<String, [usize; 4]> = Default::default();
    for (guid, row) in rows {
        let class = row.get("class").and_then(|c| c.as_str()).unwrap_or("?");
        if NOT_BODIES.contains(&class) {
            d.not_bodies += 1;
            continue;
        }
        let tally = by.entry(class.to_string()).or_default();
        let (Some(lo), Some(hi)) = (row.get("lo").and_then(v3), row.get("hi").and_then(v3)) else {
            continue;
        };
        let vol = row.get("volume").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let Some((id, mesh)) = ours.get(guid) else {
            d.missing += 1;
            tally[3] += 1;
            offs.push((0.0, format!("{class} ({guid}): no body in CreaBIM")));
            continue;
        };
        let name = model.get(*id).map(|e| e.name.clone()).unwrap_or_default();
        let Some((l, h)) = mesh.bounds() else {
            d.missing += 1;
            tally[3] += 1;
            continue;
        };
        let off = (0..3)
            .map(|k| ((l[k] as f64 - lo[k]).abs()).max((h[k] as f64 - hi[k]).abs()))
            .fold(0.0, f64::max);
        let ours_vol = mesh_volume(mesh);
        let dv = (ours_vol - vol).abs() / vol.max(1e-9);
        if off > REF_BOX {
            d.box_off += 1;
            tally[1] += 1;
            offs.push((
                off,
                format!(
                    "{class} '{name}' ({guid}): box off by {:.0} mm, volume {:+.0} %",
                    off * 1000.0,
                    (ours_vol / vol.max(1e-9) - 1.0) * 100.0
                ),
            ));
        } else if dv > REF_VOLUME && (ours_vol - vol).abs() > 1e-5 {
            d.volume_off += 1;
            tally[2] += 1;
            offs.push((
                dv * 0.001,
                format!(
                    "{class} '{name}' ({guid}): same box, volume {:+.0} % ({:.4} m³ against {:.4})",
                    (ours_vol / vol.max(1e-9) - 1.0) * 100.0,
                    ours_vol,
                    vol
                ),
            ));
        } else {
            d.same += 1;
            tally[0] += 1;
        }
    }
    d.by_class = by.into_iter().collect();
    d.reference -= d.not_bodies;
    d.extra = ours.keys().filter(|g| !rows.contains_key(*g)).count();
    offs.sort_by(|a, b| b.0.total_cmp(&a.0));
    d.worst = offs.into_iter().take(12).map(|(_, l)| l).collect();
    Some(d)
}

/// The volume a closed mesh holds (m³); an open one's is only near it.
fn mesh_volume(m: &creabim_geom::TriMesh) -> f64 {
    let p = |i: u32| {
        let q = m.positions[i as usize];
        glam::DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)
    };
    m.indices
        .as_chunks::<3>()
        .0
        .iter()
        .map(|t| p(t[0]).dot(p(t[1]).cross(p(t[2]))))
        .sum::<f64>()
        .abs()
        / 6.0
}

/// Run the bench on each file, print a report per file, and write the
/// summary table to `table` if given.
///
/// Each file is checked in a process of its own (this program, run again
/// with `--child`), so a file that crashes or hangs the importer is
/// recorded as such, with how long it ran, and the bench goes on. A crash
/// on open is a result, the first one to fix.
pub fn run(args: &[&str]) -> Result<(), String> {
    let mut files = vec![];
    let mut table = None;
    let mut timeout = 900u64;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match *a {
            "--table" => table = Some(it.next().ok_or("--table needs a file")?.to_string()),
            "--timeout" => {
                timeout = it
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--timeout needs a number of seconds")?
            }
            "--child" => {
                // one file, in this process: the report, then its row
                let f = it.next().ok_or("--child needs a file")?;
                // for testing the bench itself: die as a crash would
                if std::env::var_os("CREABIM_IFC_CHECK_ABORT").is_some() {
                    std::process::abort();
                }
                if let Some(s) = std::env::var_os("CREABIM_IFC_CHECK_STALL") {
                    let s = s.to_string_lossy().parse().unwrap_or(5);
                    std::thread::sleep(std::time::Duration::from_secs(s));
                }
                let r = check(f).unwrap_or_else(|e| failed(f, &format!("FAILED: {e}")));
                println!("{}", report_or_failure(&r));
                println!("{ROW}{}", row(&r));
                return Ok(());
            }
            f => files.push(f.to_string()),
        }
    }
    if files.is_empty() {
        return Err(
            "usage: creabim ifc-check <file.ifc>... [--table out.md] [--timeout seconds]".into(),
        );
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut rows = vec![];
    let mut bad = 0;
    for f in &files {
        let (report, row_text, ok) = in_child(&exe, f, timeout);
        println!("{report}");
        bad += usize::from(!ok);
        rows.push(row_text);
    }
    let t = format!("{}{}\n", header(), rows.join("\n"));
    println!("{t}");
    if let Some(out) = table {
        std::fs::write(&out, t).map_err(|e| format!("{out}: {e}"))?;
    }
    if bad > 0 {
        return Err(format!("{bad} files failed, crashed or hung"));
    }
    Ok(())
}

const ROW: &str = "@@ifc-check-row@@";

fn is_failure(r: &Run) -> bool {
    ["FAILED", "CRASHED", "HUNG"]
        .iter()
        .any(|p| r.schema.starts_with(p))
}

/// Check `file` in a child process: its report, its table row, and
/// whether it was checked; or a row that says how it failed.
fn in_child(exe: &Path, file: &str, timeout: u64) -> (String, String, bool) {
    use std::process::{Command, Stdio};
    let start = Instant::now();
    let child = Command::new(exe)
        .args(["ifc-check", "--child", file])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            let r = failed(file, &format!("FAILED: could not start: {e}"));
            return (report_or_failure(&r), row(&r), false);
        }
    };
    // read the pipes on their own threads, so a chatty child can't block
    let read = |p: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut s = String::new();
            if let Some(mut p) = p {
                let _ = p.read_to_string(&mut s);
            }
            s
        })
    };
    let out_t = read(child.stdout.take().map(|p| Box::new(p) as _));
    let err_t = read(child.stderr.take().map(|p| Box::new(p) as _));
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break Some(st),
            Ok(None) if start.elapsed().as_secs() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(100)),
            Err(_) => break None,
        }
    };
    let secs = start.elapsed().as_secs_f64();
    let out = out_t.join().unwrap_or_default();
    let err = err_t.join().unwrap_or_default();
    if let (Some(st), Some((report, row_text))) = (status, out.split_once(ROW)) {
        if st.success() {
            let ok = !row_text.contains("| FAILED");
            return (
                report.trim_end().to_string(),
                row_text.trim().to_string(),
                ok,
            );
        }
    }
    let last = err
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string();
    let why = match status {
        None => format!("HUNG: stopped after {secs:.0} s"),
        Some(st) => format!(
            "CRASHED: exit code {} after {secs:.1} s{}",
            st.code().map_or("none".to_string(), |c| format!("{c:#x}")),
            if last.is_empty() {
                String::new()
            } else {
                format!(": {last}")
            }
        ),
    };
    let mut r = failed(file, &why);
    r.bytes = std::fs::metadata(file).map_or(0, |m| m.len() as usize);
    (report_or_failure(&r), row(&r), false)
}

fn report_or_failure(r: &Run) -> String {
    if is_failure(r) {
        format!(
            "## {}\n\n**{}** ({:.1} MB)\n",
            name(&r.file),
            r.schema,
            r.bytes as f64 / 1_048_576.0
        )
    } else {
        report(r)
    }
}

fn failed(file: &str, e: &str) -> Run {
    Run {
        file: file.to_string(),
        bytes: 0,
        schema: e.to_string(),
        source_objects: 0,
        imported: 0,
        meshes: 0,
        warnings: 0,
        measures: vec![],
        store_changed: 0,
        store_missing: 0,
        export_noise: 0,
        ids_kept: 0,
        records_same: 0,
        records: 0,
        edit: None,
        geometry: (0, 0, 0, 0.0),
        geometry_worst: vec![],
        reference: None,
        times: [0.0; 4],
        classes_in: vec![],
        classes_model: vec![],
    }
}

fn opts() -> ExportOptions {
    // a fixed timestamp, so two exports differ only where the model does
    ExportOptions {
        project_name: "ifc-check".into(),
        ..Default::default()
    }
}

/// Check one file.
pub fn check(path: &str) -> Result<Run, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let source = parse(&text).map_err(|e| e.to_string())?;
    let census_in = Census::of(&source);

    let t = Instant::now();
    let imported = import_ifc(&text).map_err(|e| e.to_string())?;
    let t_import = t.elapsed().as_secs_f64();
    let m1 = imported.model;

    // the .creabim round trip, in a folder of its own
    let dir = std::env::temp_dir().join(format!("creabim-ifc-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stem = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let db = dir.join(format!("{stem}.creabim"));
    let t = Instant::now();
    creabim_store::save(&db, &Document::from_model(m1.clone())).map_err(|e| e.to_string())?;
    let t_save = t.elapsed().as_secs_f64();
    let t = Instant::now();
    let mut doc = creabim_store::open(&db).map_err(|e| e.to_string())?;
    let t_open = t.elapsed().as_secs_f64();
    let _ = std::fs::remove_file(&db);
    let _ = std::fs::remove_dir(&dir);
    let (store_changed, store_missing) = model_diff(&m1, doc.model());

    let t = Instant::now();
    let out = export_ifc4(doc.model(), &opts());
    let t_export = t.elapsed().as_secs_f64();
    let written = parse(&out).map_err(|e| format!("export: {e}"))?;
    let census_out = Census::of(&written);
    let ids_kept = source
        .entities
        .iter()
        .filter(|(id, e)| written.get(**id).is_some_and(|w| w.class == e.class))
        .count();
    let records_same = {
        let lines: std::collections::HashSet<&str> = out
            .lines()
            .map(str::trim)
            .filter(|l| l.starts_with('#'))
            .collect();
        text.lines()
            .map(str::trim)
            .filter(|l| l.starts_with('#') && lines.contains(l))
            .count()
    };
    let export_noise = line_diff(&out, &export_ifc4(doc.model(), &opts()));

    // the one-edit test: one wall 100 mm taller
    let wall = doc
        .model()
        .iter()
        .find(|e| matches!(&e.kind, ElementKind::Wall(w) if w.top.is_unconnected()))
        .cloned();
    let edit = wall.and_then(|before| {
        let mut after = before.clone();
        if let ElementKind::Wall(w) = &mut after.kind {
            w.height += 0.1;
        }
        let label = format!("{} '{}'", before.ifc_class(), before.name);
        doc.commit(Transaction::new("ifc-check edit").replace(before, after))
            .ok()
            .map(|_| {
                let edited = export_ifc4(doc.model(), &opts());
                (label, out.lines().count(), line_diff(&out, &edited))
            })
    });

    // geometry: import the export again and compare each element's box
    let (geometry, geometry_worst) = match import_ifc(&out) {
        Ok(r) => shape_diff(&m1, &r.model),
        Err(_) => ((m1.len(), m1.len(), 0, 0.0), vec![]),
    };

    let mut classes_model: HashMap<String, (usize, usize)> = HashMap::new();
    for e in m1.iter() {
        let c = classes_model.entry(e.ifc_class().to_string()).or_default();
        c.0 += 1;
        c.1 += usize::from(matches!(e.kind, ElementKind::FreeMesh(_)));
    }
    let mut classes_model: Vec<(String, usize, usize)> = classes_model
        .into_iter()
        .map(|(c, (n, m))| (c, n, m))
        .collect();
    classes_model.sort();

    Ok(Run {
        file: path.to_string(),
        bytes: text.len(),
        schema: census_in.schema.clone(),
        source_objects: census_in.objects.len(),
        imported: m1.len(),
        meshes: m1
            .iter()
            .filter(|e| matches!(e.kind, ElementKind::FreeMesh(_)))
            .count(),
        warnings: imported.details.len().max(imported.warnings.len()),
        measures: census_in.compare(&census_out),
        store_changed,
        store_missing,
        export_noise,
        ids_kept,
        records_same,
        records: source.entities.len(),
        edit,
        geometry,
        geometry_worst,
        reference: reference_diff(path, &m1),
        times: [t_import, t_save, t_open, t_export],
        classes_in: census_in.classes().into_iter().collect(),
        classes_model,
    })
}

/// Elements that differ, and that are missing, after a round trip.
fn model_diff(a: &Model, b: &Model) -> (usize, usize) {
    let (mut changed, mut missing) = (0, 0);
    for e in a.iter() {
        match b.get(e.id) {
            Some(f) if f == e => {}
            Some(_) => changed += 1,
            None => missing += 1,
        }
    }
    (changed, missing + b.len().saturating_sub(a.len()))
}

/// Lines in either text and not the other, counted with repeats; the
/// order of lines is ignored (STEP numbers each record, so a record that
/// only moved still shows, by its number).
pub fn line_diff(a: &str, b: &str) -> usize {
    let mut n: HashMap<&str, i64> = HashMap::new();
    for l in a.lines() {
        *n.entry(l).or_default() += 1;
    }
    for l in b.lines() {
        *n.entry(l).or_default() -= 1;
    }
    n.values().map(|v| v.unsigned_abs() as usize).sum()
}

/// Each element's box in `a` against the same element in `b`: how many
/// were compared, gone, off by more than 1 mm, and the largest offset.
fn shape_diff(a: &Model, b: &Model) -> ((usize, usize, usize, f64), Vec<String>) {
    use creabim_geom::scene::{build_scene_for_exchange, element_meshes};
    let boxes = |m: &Model| -> HashMap<_, _> {
        element_meshes(&build_scene_for_exchange(m))
            .into_iter()
            .filter_map(|(id, mesh)| mesh.bounds().map(|b| (id, b)))
            .collect()
    };
    let (ba, bb) = (boxes(a), boxes(b));
    let (mut gone, mut off, mut worst) = (0, 0, 0.0f64);
    let mut offs = vec![];
    for (id, (lo, hi)) in &ba {
        match bb.get(id) {
            None => gone += 1,
            Some((lo2, hi2)) => {
                let d = (*lo - *lo2)
                    .abs()
                    .max_element()
                    .max((*hi - *hi2).abs().max_element()) as f64;
                worst = worst.max(d);
                if d > 0.001 {
                    off += 1;
                    offs.push((d, *id));
                }
            }
        }
    }
    offs.sort_by(|x, y| y.0.total_cmp(&x.0));
    let named = offs
        .iter()
        .take(8)
        .map(|(d, id)| match a.get(*id) {
            Some(e) => format!(
                "{} '{}' ({}) {:.1} mm",
                e.ifc_class(),
                e.name,
                id.ifc_guid(),
                d * 1000.0
            ),
            None => format!("{} {:.1} mm", id.ifc_guid(), d * 1000.0),
        })
        .collect();
    ((ba.len(), gone, off, worst), named)
}

fn name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

/// The per-file report, in Markdown.
pub fn report(r: &Run) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "## {}\n", name(&r.file));
    let _ = writeln!(
        s,
        "{} · {:.0} KB · {} objects in the file · {} elements imported ({} meshes) · {} import notes\n",
        r.schema,
        r.bytes as f64 / 1024.0,
        r.source_objects,
        r.imported,
        r.meshes,
        r.warnings
    );
    let _ = writeln!(s, "| What | In the file | Kept | Changed | Lost | Added |");
    let _ = writeln!(s, "|---|---:|---:|---:|---:|---:|");
    for m in &r.measures {
        let _ = writeln!(
            s,
            "| {} | {} | {} ({:.0} %) | {} | {} | {} |",
            m.name,
            m.source,
            m.kept,
            m.kept_pct(),
            m.changed,
            m.lost,
            m.added
        );
    }
    let _ = writeln!(s);
    for m in r.measures.iter().filter(|m| m.lost > 0 || m.changed > 0) {
        if !m.lost_examples.is_empty() {
            let _ = writeln!(s, "- {} lost, e.g. {}", m.name, m.lost_examples.join("; "));
        }
        if !m.changed_examples.is_empty() {
            let _ = writeln!(
                s,
                "- {} changed, e.g. {}",
                m.name,
                m.changed_examples.join("; ")
            );
        }
    }
    let _ = writeln!(s);
    let _ = writeln!(
        s,
        "- `.creabim` save and open: {} elements changed, {} missing",
        r.store_changed, r.store_missing
    );
    let _ = writeln!(
        s,
        "- Two exports of the same model differ by {} lines",
        r.export_noise
    );
    let pct = |n: usize| 100.0 * n as f64 / r.records.max(1) as f64;
    let _ = writeln!(
        s,
        "- ifcmerge: {} of {} `#id`s come back holding the same class ({:.0} %); {} records written back byte for byte ({:.0} %)",
        r.ids_kept,
        r.records,
        pct(r.ids_kept),
        r.records_same,
        pct(r.records_same)
    );
    match &r.edit {
        Some((what, lines, changed)) => {
            let _ = writeln!(
                s,
                "- One edit ({what} 100 mm taller): {changed} of {lines} lines changed"
            );
        }
        None => {
            let _ = writeln!(s, "- One edit: no native wall with a free top to edit");
        }
    }
    let (n, gone, off, worst) = r.geometry;
    let _ = writeln!(
        s,
        "- Shape after export and import again: {n} elements, {gone} gone, {off} off by more than 1 mm (largest {:.1} mm)",
        worst * 1000.0
    );
    for w in &r.geometry_worst {
        let _ = writeln!(s, "  - {w}");
    }
    if let Some(d) = &r.reference {
        let _ = writeln!(
            s,
            "- **Against {} (Bonsai's engine):** {} of {} elements the same (box within {:.0} mm, volume within {:.0} %), {} with the box off, {} with the volume off, {} missing a body; {} bodies only in CreaBIM",
            d.engine, d.same, d.reference, REF_BOX * 1000.0, REF_VOLUME * 100.0, d.box_off, d.volume_off, d.missing, d.extra
        );
        let _ = writeln!(
            s,
            "  - not compared: {} openings, annotations and reference lines",
            d.not_bodies
        );
        let _ = writeln!(
            s,
            "
  | Class | Same | Box off | Volume off | Missing |
  |---|---:|---:|---:|---:|"
        );
        for (c, [a, b, v, m]) in &d.by_class {
            let _ = writeln!(s, "  | {c} | {a} | {b} | {v} | {m} |");
        }
        let _ = writeln!(
            s,
            "
  The furthest off:"
        );
        for w in &d.worst {
            let _ = writeln!(s, "  - {w}");
        }
    }
    let [a, b, c, d] = r.times;
    let _ = writeln!(
        s,
        "- Times: import {a:.2} s, save {b:.2} s, open {c:.2} s, export {d:.2} s\n"
    );
    let _ = writeln!(s, "<details><summary>Classes</summary>\n");
    let _ = writeln!(s, "| In the file | n | | Imported as | n | meshes |");
    let _ = writeln!(s, "|---|---:|---|---|---:|---:|");
    let rows = r.classes_in.len().max(r.classes_model.len());
    for i in 0..rows {
        let (a, n) = r
            .classes_in
            .get(i)
            .map(|(c, n)| (c.as_str(), n.to_string()))
            .unwrap_or(("", String::new()));
        let (b, m, k) = r
            .classes_model
            .get(i)
            .map(|(c, n, k)| (c.as_str(), n.to_string(), k.to_string()))
            .unwrap_or(("", String::new(), String::new()));
        let _ = writeln!(s, "| {a} | {n} | | {b} | {m} | {k} |");
    }
    let _ = writeln!(s, "\n</details>\n");
    s
}

fn header() -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "| File | Schema | Objects | Imported (meshes) | GlobalIds kept | Psets kept | Props kept (same value) | Class. | Groups | Docs | Drawings | Sheets | Types | Storeys | Hosted D/W | .creabim | ids kept | Records same | Export noise | One edit | Shape | Import / export s |"
    );
    let _ = writeln!(s, "|---|---|{}", "---:|".repeat(20));
    s
}

/// One file's row of the summary table.
pub fn row(r: &Run) -> String {
    let pick = |r: &Run, n: &str| {
        r.measures
            .iter()
            .find(|m| m.name == n)
            .map(|m| {
                if m.source == 0 {
                    "–".to_string()
                } else {
                    format!("{}/{}", m.kept, m.source)
                }
            })
            .unwrap_or_else(|| "–".into())
    };
    let mut s = String::new();
    if is_failure(r) {
        return format!("| {} | {} |{}", name(&r.file), r.schema, " |".repeat(20));
    }
    let props = r
        .measures
        .iter()
        .find(|m| m.name == "properties (value)")
        .map(|m| {
            if m.source == 0 {
                "–".into()
            } else {
                format!("{}/{} ({})", m.kept, m.source, m.kept - m.changed)
            }
        })
        .unwrap_or_default();
    let store = if r.store_changed + r.store_missing == 0 {
        "same".to_string()
    } else {
        format!("{} differ", r.store_changed + r.store_missing)
    };
    let edit = r
        .edit
        .as_ref()
        .map(|(_, l, c)| format!("{c}/{l}"))
        .unwrap_or_else(|| "–".into());
    let (n, gone, off, _) = r.geometry;
    let ids_kept = format!(
        "{:.0} %",
        100.0 * r.ids_kept as f64 / r.records.max(1) as f64
    );
    let records_same = format!(
        "{:.0} %",
        100.0 * r.records_same as f64 / r.records.max(1) as f64
    );
    let _ = writeln!(
        s,
        "| {} | {} | {} | {} ({}) | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {}/{} ok | {:.2} / {:.2} |",
        name(&r.file),
        r.schema,
        r.source_objects,
        r.imported,
        r.meshes,
        pick(r, "objects (GlobalId, class)"),
        pick(r, "property sets"),
        props,
        pick(r, "classification refs"),
        pick(r, "group memberships"),
        pick(r, "documents on objects"),
        pick(r, "Bonsai drawings"),
        pick(r, "Bonsai sheets"),
        pick(r, "type of each object"),
        pick(r, "storey of each element"),
        pick(r, "doors and windows in hosts"),
        store,
        ids_kept,
        records_same,
        r.export_noise,
        edit,
        n - gone - off,
        n,
        r.times[0],
        r.times[3],
    );
    s.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_diff_counts_both_sides() {
        assert_eq!(line_diff("a\nb\nc", "a\nb\nc"), 0);
        assert_eq!(line_diff("a\nb\nc", "a\nB\nc"), 2);
        assert_eq!(line_diff("a\nb", "b\na"), 0);
    }
}

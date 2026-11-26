//! What the model does not show, drafted in each detail: plates, headers,
//! flashings, membranes and insulation at the junctions, and the notes
//! that name every layer.
//!
//! Only pieces that sit on what the model draws are kept. The eave's
//! fascia, gutter and soffit, the canopy's ledger, and the sill plate and
//! base flashing where the section passes through the slider were drafted
//! at fixed places the model did not bear out, and are left out until the
//! model carries them (owner, 2026-11-22). The sheet says the details are
//! work in progress.
//!
//! In the wall section's drawing the wall's centre line is `w` along it,
//! the outside to the right.

use super::drawings::{detail_boxes, wall_at};
use super::*;
use crate::*;
use glam::DVec2;

pub fn draft(b: &mut Builder) {
    use DetailKind as K;
    let boxes = detail_boxes();
    let (base, floor, window, eaves) = (boxes[0].0, boxes[1].0, boxes[2].0, boxes[3].0);
    let head_box = eaves;
    let w = wall_at();
    // faces: inside, the studs' two faces, outside
    let (inner, stud_in, stud_out, outer) = (w - H, w - H + 0.017, w + H - FRAME_OUT, w + H);
    let mid = (stud_in + stud_out) / 2.0;
    let top = STOREY + PLATE;
    let (sill, head) = (super::drawings::SILL, super::drawings::HEAD);
    // the framing's top: under the floor's oak, plywood and joists
    let frame_top = STOREY - 0.34;
    let slider = ft(8.0);
    let pts = |q: &[(f64, f64)]| q.iter().map(|&(x, y)| DVec2::new(x, y)).collect::<Vec<_>>();
    let items: Vec<(&str, DetailKind, f64, Vec<DVec2>)> = vec![
        // the foundation: the slab's thermal break, a drain at the footing
        (
            base,
            K::Region {
                hatch: Hatch::Cross,
            },
            0.0,
            pts(&[
                (w - 0.127, 0.0),
                (w - 0.076, 0.0),
                (w - 0.076, -0.12),
                (w - 0.127, -0.12),
            ]),
        ),
        (
            base,
            K::Region {
                hatch: Hatch::Diagonal,
            },
            0.0,
            pts(&[
                (w + 0.13, -1.0),
                (w + 0.5, -1.0),
                (w + 0.5, -1.22),
                (w + 0.13, -1.22),
            ]),
        ),
        (
            base,
            K::BreakLine,
            0.0,
            pts(&[(w - 0.68, 0.12), (w - 0.68, -0.5)]),
        ),
        (
            base,
            K::BreakLine,
            0.0,
            pts(&[(inner - 0.08, 0.4), (outer + 0.08, 0.4)]),
        ),
        // the floor's edge over the slider: header, rim board, rim
        // insulation, and the flashings over the canopy and the door
        (
            floor,
            K::Blocking,
            0.14,
            pts(&[(mid, slider + 0.02), (mid, frame_top - 0.08)]),
        ),
        (
            floor,
            K::Blocking,
            0.076,
            pts(&[(stud_in, frame_top - 0.038), (stud_out, frame_top - 0.038)]),
        ),
        (
            floor,
            K::Blocking,
            0.038,
            pts(&[
                (stud_out - 0.019, STOREY - FLOOR + 0.02),
                (stud_out - 0.019, STOREY - 0.04),
            ]),
        ),
        (
            floor,
            K::Batt,
            0.09,
            pts(&[
                (stud_out - 0.09, STOREY - FLOOR + 0.03),
                (stud_out - 0.09, STOREY - 0.05),
            ]),
        ),
        (
            floor,
            K::Flashing,
            0.002,
            pts(&[
                (outer - 0.038, structure::CANOPY + 0.3),
                (outer - 0.038, structure::CANOPY + 0.11),
                (outer + 0.12, structure::CANOPY + 0.105),
            ]),
        ),
        (
            floor,
            K::Flashing,
            0.002,
            pts(&[
                (outer - 0.06, slider + 0.012),
                (outer + 0.012, slider + 0.004),
                (outer + 0.012, slider - 0.03),
            ]),
        ),
        (
            floor,
            K::BreakLine,
            0.0,
            pts(&[(w - 0.68, STOREY + 0.1), (w - 0.68, STOREY - 0.5)]),
        ),
        (
            floor,
            K::BreakLine,
            0.0,
            pts(&[(outer + 0.5, STOREY - 0.3), (outer + 0.5, STOREY - 0.75)]),
        ),
        // the window: header, sill pan, flashings with drips, a stool inside
        // the sill: a double rough sill, a sloped pan under the window,
        // a metal sill with a drip outside, an oak stool inside
        (
            window,
            K::Blocking,
            0.038,
            pts(&[(stud_in, sill - 0.052), (stud_out, sill - 0.052)]),
        ),
        (
            window,
            K::Blocking,
            0.038,
            pts(&[(stud_in, sill - 0.09), (stud_out, sill - 0.09)]),
        ),
        (
            window,
            K::Membrane,
            0.0,
            pts(&[(stud_in, sill - 0.028), (outer - 0.02, sill - 0.036)]),
        ),
        (
            window,
            K::Flashing,
            0.002,
            pts(&[
                (mid + 0.03, sill - 0.006),
                (outer + 0.03, sill - 0.02),
                (outer + 0.03, sill - 0.05),
            ]),
        ),
        (
            window,
            K::Blocking,
            0.022,
            pts(&[(inner - 0.03, sill - 0.012), (mid - 0.03, sill - 0.012)]),
        ),
        (
            window,
            K::BreakLine,
            0.0,
            pts(&[(inner - 0.08, sill - 0.3), (outer + 0.08, sill - 0.3)]),
        ),
        (
            window,
            K::BreakLine,
            0.0,
            pts(&[(inner - 0.08, sill + 0.3), (outer + 0.08, sill + 0.3)]),
        ),
        // the head: two 2×8 plies with foam between, a flashing with a
        // drip over the window; then the plates. The roof runs out plain,
        // as the model has it
        (
            head_box,
            K::Blocking,
            0.038,
            pts(&[
                (stud_in + 0.019, head + 0.03),
                (stud_in + 0.019, head + 0.214),
            ]),
        ),
        (
            head_box,
            K::Blocking,
            0.038,
            pts(&[
                (stud_out - 0.019, head + 0.03),
                (stud_out - 0.019, head + 0.214),
            ]),
        ),
        (
            head_box,
            K::Region {
                hatch: Hatch::Cross,
            },
            0.0,
            pts(&[
                (stud_in + 0.038, head + 0.03),
                (stud_out - 0.038, head + 0.03),
                (stud_out - 0.038, head + 0.214),
                (stud_in + 0.038, head + 0.214),
            ]),
        ),
        (
            head_box,
            K::Flashing,
            0.002,
            pts(&[
                (outer - 0.06, head + 0.012),
                (outer + 0.012, head + 0.004),
                (outer + 0.012, head - 0.03),
            ]),
        ),
        (
            head_box,
            K::Blocking,
            0.076,
            pts(&[(stud_in, top - 0.038), (stud_out, top - 0.038)]),
        ),
        (
            head_box,
            K::BreakLine,
            0.0,
            pts(&[(inner - 0.08, head - 0.25), (outer + 0.08, head - 0.25)]),
        ),
    ];
    let mut tx = Transaction::new("Ridgeline House: detail components");
    for (detail, kind, width, path) in items {
        let mut item = DetailItem::new(b.id(detail), kind, path);
        item.width = width;
        tx = tx.insert(Element::new(kind.label(), ElementKind::Detail(item)));
    }
    let wall_layers = "3/4\" CEDAR, VERTICAL\n3/4\" VENTED CAVITY ON STRAPPING\n2\" MINERAL WOOL BOARD, R-8\nWRB ON 7/16\" SHEATHING\n2×6 AT 16\" O.C., R-23 BATTS\nSMART VAPOUR RETARDER\n5/8\" GYPSUM BOARD";
    #[allow(clippy::type_complexity)]
    let notes: Vec<(&str, (f64, f64), &str, Option<(f64, f64)>)> = vec![
        (base, (w + 0.8, -0.1), "FINISHED GRADE, 8\" BELOW FLOOR\nslope 5% away for 10 ft", Some((outer + 0.3, -0.2))),
        (base, (w + 0.8, -0.55), "8\" BOARD-FORMED STEM WALL\n#4 AT 16\" O.C. EACH WAY", Some((w + 0.03, -0.6))),
        (base, (w + 0.8, -1.08), "4\" PERFORATED DRAIN IN\nWASHED STONE, FILTER FABRIC", Some((w + 0.3, -1.1))),
        (base, (w + 0.8, -1.36), "FOOTING 20\" × 10\", 2 #5 CONT.\nUNDERSIDE 50\" BELOW GRADE", Some((w + 0.1, -1.36))),
        (base, (w - 1.2, -0.3), "3/4\" OAK ON 4\" SLAB, RADIANT\n15 MIL VAPOUR BARRIER\n4\" XPS, R-20\n6\" DRAINAGE GRAVEL", Some((w - 0.4, -0.15))),
        (base, (w - 1.2, -0.75), "2\" XPS TO FOOTING, R-10\nTHERMAL BREAK AT SLAB EDGE", Some((w - 0.1, -0.7))),
        (floor, (outer + 0.2, STOREY + 0.36), "SELF-ADHERED FLASHING\nLAPPED UNDER WRB", Some((outer - 0.03, structure::CANOPY + 0.25))),
        (floor, (outer + 0.2, STOREY + 0.05), "CANOPY: STANDING SEAM\nON PURLINS", Some((outer + 0.25, structure::CANOPY + 0.05))),
        (floor, (outer + 0.2, slider - 0.1), "HEAD FLASHING WITH DRIP\nLIFT-SLIDE DOOR, U-0.17", Some((outer + 0.01, slider - 0.01))),
        (floor, (w - 1.2, STOREY + 0.36), "3/4\" OAK ON 3/4\" T&G PLYWOOD\n11 7/8\" I-JOISTS AT 16\" O.C.\n5/8\" GYPSUM BOARD", Some((w - 0.4, STOREY - 0.15))),
        (floor, (w - 1.2, STOREY - 0.5), "1 1/8\" RIM BOARD, R-15 BATT\nBEHIND, SEALED TO PLATES", Some((stud_out - 0.06, STOREY - 0.2))),
        (floor, (w - 1.2, STOREY - 0.76), "(3) 1 3/4\" × 9 1/4\" LVL HEADER", Some((mid, slider + 0.12))),
        (window, (outer + 0.14, sill + 0.22), "TRIPLE-GLAZED WINDOW, U-0.17\nBRONZE ALUMINIUM-CLAD, SET IN\nLINE WITH THE EXTERIOR INSULATION", Some((mid + 0.03, sill + 0.15))),
        (window, (outer + 0.14, sill - 0.06), "SLOPED SILL PAN, METAL SILL\nWITH DRIP", Some((outer + 0.03, sill - 0.03))),
        (window, (w - 0.58, sill + 0.14), "OAK STOOL", Some((inner - 0.01, sill - 0.01))),
        (window, (w - 0.58, sill - 0.16), "DOUBLE 2×6 ROUGH SILL", Some((mid - 0.02, sill - 0.07))),
        (window, (outer + 0.14, sill - 0.14), wall_layers, Some((w + 0.02, sill - 0.2))),
        (eaves, (outer + 0.35, top + 0.85), "STANDING SEAM ON HIGH-TEMP\nUNDERLAYMENT, 5/8\" SHEATHING\n1 1/2\" VENT SPACE\n14\" I-JOISTS, DENSE-PACK R-52\n1 1/2\" POLYISO, R-9, TAPED\n5/8\" GYPSUM BOARD", Some((outer - 0.3, top + 0.35))),
        (eaves, (w - 0.72, top + 0.08), "DOUBLE 2×6 TOP PLATE\nHURRICANE TIE EACH RAFTER", Some((mid, top - 0.04))),
        (eaves, (w - 0.72, head - 0.12), "(2) 2×8 HEADER, 2 1/2\" FOAM\nBETWEEN THE PLIES, R-10", Some((mid, head + 0.12))),
        (eaves, (outer + 0.14, head - 0.08), "HEAD FLASHING WITH END DAMS", Some((outer + 0.01, head))),
    ];
    for (detail, at, text, leader) in notes {
        let mut n = Note::new(b.id(detail), DVec2::new(at.0, at.1), text);
        n.leader = leader.map(|l| HostPoint::free(DVec2::new(l.0, l.1)));
        tx = tx.insert(Element::new("Note", ElementKind::Note(n)));
    }
    b.commit(tx);
}

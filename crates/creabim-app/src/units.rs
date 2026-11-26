//! The project's units in the app. The app sets them once a frame from the
//! model ([`set`]); every field, readout and status message then formats
//! and reads lengths through this module, so they all follow the project's
//! units without passing them round. The model itself is always metres.

use creabim_core::Units;
use eframe::egui;
use std::cell::Cell;

thread_local! {
    static CURRENT: Cell<Units> = Cell::new(Units::metric());
}

/// Use these units from now on (called each frame with the project's).
pub fn set(u: Units) {
    CURRENT.with(|c| c.set(u));
}

pub fn get() -> Units {
    CURRENT.with(|c| c.get())
}

/// A length with its unit: `1067 mm`, `3'-6 1/2"`.
pub fn len(metres: f64) -> String {
    get().length(metres)
}

/// A signed height, as level elevations are written: `+3000`, `+9'-10"`.
pub fn elevation(metres: f64) -> String {
    let u = get();
    let text = u.elevation(metres);
    if u.is_imperial() {
        text
    } else {
        format!("{text} {}", unit_symbol(&u))
    }
}

/// The metric unit lengths are written in (`mm`, `cm` or `m`).
pub fn symbol() -> &'static str {
    unit_symbol(&get())
}

fn unit_symbol(u: &Units) -> &'static str {
    match u.length {
        creabim_core::LengthUnit::Centimetres => "cm",
        creabim_core::LengthUnit::Metres => "m",
        _ => "mm",
    }
}

/// An area: `12.50 m²`, `134.55 ft²`.
pub fn area(square_metres: f64) -> String {
    get().area(square_metres)
}

/// A volume in the project's units: `3.20 m³` or `113.00 ft³`.
pub fn volume(cubic_metres: f64) -> String {
    get().volume(cubic_metres)
}

/// A typed length, metres.
pub fn parse(text: &str) -> Option<f64> {
    get().parse(text)
}

/// A field for a length in metres, shown and typed in the project's units.
/// Any unit can be typed (`3'-6 1/2"`, `1067`, `1.2 m`); dragging moves it
/// about a centimetre (or half an inch) a step.
pub fn drag(v: &mut f64) -> egui::DragValue<'_> {
    let u = get();
    let speed = if u.is_imperial() { 0.0127 } else { 0.01 };
    egui::DragValue::new(v)
        .speed(speed)
        .custom_formatter(move |v, _| u.length(v))
        .custom_parser(move |s| u.parse(s))
        // typed, a length is taken once it is whole (Enter or leaving the
        // field), not at every key: 4' of 4'-6" is a length too, and the
        // panel drawn again under the cursor threw it out (MU141). Dragged,
        // it still changes as it moves
        .update_while_editing(false)
}

/// Inches to the nearest 1/16, as a sheet is measured in the US
/// ("3 15/16\"").
fn inches(mm: f64) -> String {
    inches_in(mm, 16)
}

/// Inches to the nearest 1/`parts` ("3/32\"" with 64).
fn inches_in(mm: f64, parts: i64) -> String {
    let n = (mm / 25.4 * parts as f64).round() as i64;
    let (sign, s) = if n < 0 { ("-", -n) } else { ("", n) };
    let (whole, mut num) = (s / parts, s % parts);
    let mut den = parts;
    while num != 0 && num % 2 == 0 {
        num /= 2;
        den /= 2;
    }
    match (whole, num) {
        (_, 0) => format!("{sign}{whole}\""),
        (0, _) => format!("{sign}{num}/{den}\""),
        _ => format!("{sign}{whole} {num}/{den}\""),
    }
}

/// A length on paper, kept in millimetres: shown in inches for an
/// imperial project, in millimetres else; any unit can be typed (4", 4in,
/// 0'-4", 100 mm). MU91: sheet fields stayed in mm in a ft-in project.
pub fn paper(mm: &mut f64) -> egui::DragValue<'_> {
    paper_field(mm, 16)
}

/// A small length on paper (a text height, an arrow), kept in millimetres:
/// to 1/64 inch in an imperial project ("3/32\"", "1/8\""), as US text
/// heights are written.
pub fn paper_small(mm: &mut f64) -> egui::DragValue<'_> {
    paper_field(mm, 64)
}

/// A paper length as words: "3 15/16\"" or "100 mm".
pub fn paper_text(mm: f64) -> String {
    if get().is_imperial() {
        inches(mm)
    } else {
        let t = format!("{mm:.1}");
        format!("{} mm", t.trim_end_matches(".0"))
    }
}

/// A model length kept in millimetres (a pipe, a pattern's spacing), shown
/// and typed in the project's units.
pub fn model_mm(mm: &mut f64) -> egui::DragValue<'_> {
    let u = get();
    egui::DragValue::new(mm)
        .speed(if u.is_imperial() { 1.5875 } else { 1.0 })
        .custom_formatter(move |v, _| u.length(v / 1000.0))
        .custom_parser(move |s| u.parse(s).map(|m| m * 1000.0))
}

fn paper_field(mm: &mut f64, parts: i64) -> egui::DragValue<'_> {
    let u = get();
    let imperial = u.is_imperial();
    egui::DragValue::new(mm)
        .speed(if imperial {
            25.4 / parts as f64
        } else if parts > 16 {
            0.05
        } else {
            0.5
        })
        .custom_formatter(move |v, _| {
            if imperial {
                inches_in(v, parts)
            } else {
                let t = format!("{v:.2}");
                format!("{} mm", t.trim_end_matches('0').trim_end_matches('.'))
            }
        })
        .custom_parser(move |s| {
            let t = s.trim();
            // a bare number is in the field's own unit
            if let Ok(n) = t.parse::<f64>() {
                return Some(if imperial { n * 25.4 } else { n });
            }
            u.parse(t).map(|m| m * 1000.0)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_show_and_read_the_project_units() {
        set(Units::imperial());
        assert_eq!(len(1.0668), "3'-6\"");
        assert!((parse("3'-6\"").unwrap() - 1.0668).abs() < 1e-9);
        assert_eq!(elevation(3.0), "+9'-10 1/8\"");
        set(Units::metric());
        assert_eq!(len(1.0668), "1067 mm");
        assert_eq!(elevation(3.0), "+3000 mm");
        assert_eq!(area(2.0), "2.00 m²");
        assert_eq!(super::inches(100.0), "3 15/16\"");
        assert_eq!(super::inches(12.7), "1/2\"");
        assert_eq!(super::inches_in(2.38, 64), "3/32\"");
    }
}

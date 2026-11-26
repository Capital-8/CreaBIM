//! A level on the site, as a field: typed from the datum (100.45) or
//! from the project's zero (+0.45), with the other reading in grey
//! under it. One height is kept; which way it reads is the project's,
//! set once in the Project tab.

use creabim_core::{LevelReading, Model};
use eframe::egui;

/// A Level field for `z`, a height above the project's zero in metres.
/// True when it was changed.
pub fn field(ui: &mut egui::Ui, model: &Model, label: &str, z: &mut f64) -> bool {
    let reading = creabim_core::units::level_reading(model);
    let shift = creabim_core::units::level_shift(model);
    let per = creabim_core::units::level_unit(model);
    let unit = if per == 1.0 { " m" } else { " ft" };
    let mut shown = (*z + shift) / per;
    let (other, tip) = match reading {
        LevelReading::Datum => (
            creabim_core::units::level_from_zero(model, *z),
            "The same level above the project's zero",
        ),
        LevelReading::Zero => (
            creabim_core::site_level(model, *z),
            "The same level above the datum, as site drawings write it",
        ),
    };
    let changed = crate::props::row(ui, label, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let w = ui.available_width().floor().max(48.0);
            let h = ui.spacing().interact_size.y;
            let mut drag = egui::DragValue::new(&mut shown)
                .speed(0.01)
                .range(-1000.0..=10000.0)
                .fixed_decimals(2)
                .suffix(unit);
            if reading == LevelReading::Zero {
                drag = drag.custom_formatter(|v, _| format!("{v:+.2}"));
            }
            let r = ui.add_sized([w, h], drag);
            r.changed()
        })
        .inner
    });
    if changed {
        *z = shown * per - shift;
    }
    // the same level read the other way, worked out: in the block's plate
    // (AMD-015, AMD-018)
    crate::props::worked_later(
        ui,
        "site.element",
        &[crate::props::Worked::new(
            match reading {
                LevelReading::Datum => "From zero",
                LevelReading::Zero => "From datum",
            },
            other,
        )],
    );
    let _ = tip;
    changed
}

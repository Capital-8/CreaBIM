//! The 3D view's display buttons (Visuals REV02, approved 2026-11-20).
//!
//! Four fixed buttons in the view's header, as Blender's: Line, Solid,
//! Style and Rendered. Each shows the look of its family used last; which
//! style and which renderer are chosen in Properties, not in the header.
//! The caret beside them holds the lit button's one option. Grids, levels
//! and dimensions are left off a Style or Rendered look unless asked for.

use super::CreaBimApp;
use crate::render3d::{Family, RenderMode};
use crate::ui_model::Item;
use eframe::egui;

impl CreaBimApp {
    /// The look of `family` the 3D view shows when its button is pressed:
    /// the one used last.
    pub(crate) fn family_look(&self, family: Family) -> RenderMode {
        let s = &self.shell.settings;
        let kept = match family {
            Family::Line => s.look_line,
            Family::Solid => s.look_solid,
            Family::Style => s.look_style,
            Family::Rendered => s.look_rendered,
        };
        // (a settings file edited by hand may hold another family's look)
        if kept.family() == family {
            kept
        } else {
            family.first()
        }
    }

    /// Show `look` in the 3D view, and keep it as its family's.
    pub(crate) fn set_look(&mut self, look: RenderMode) {
        let s = &mut self.shell.settings;
        s.render_3d = look;
        match look.family() {
            Family::Line => s.look_line = look,
            Family::Solid => s.look_solid = look,
            Family::Style => s.look_style = look,
            Family::Rendered => s.look_rendered = look,
        }
        s.store();
        self.geometry_key = None;
        self.status = format!("3D view: {}. {}.", look.label(), look.hint());
    }

    /// What the lit display button's caret lists: that button's own
    /// looks and nothing else, the one shown ticked (MU112: the overlay
    /// switch and Save image there "break a system of thinking"; they are
    /// in Properties).
    pub(crate) fn display_items(&self) -> Vec<Item> {
        let now = self.shell.settings.render_3d;
        RenderMode::ALL
            .iter()
            .enumerate()
            .filter(|(_, m)| m.family() == now.family())
            .map(|(i, m)| {
                Item::new(format!("render3d:{i}"), m.label())
                    .checked(now == *m)
                    .hint(m.hint())
            })
            .collect()
    }

    /// A display button or one of the caret's options; whether `head`
    /// (with `arg` after its colon) was one.
    pub(crate) fn display_action(&mut self, head: &str, arg: &str) -> bool {
        let now = self.shell.settings.render_3d;
        match head {
            "display" => {
                if let Some(f) = Family::of_key(arg) {
                    let look = self.family_look(f);
                    self.set_look(look);
                }
            }
            "display.hidden" => self.set_look(if now == RenderMode::HiddenLine {
                RenderMode::Wireframe
            } else {
                RenderMode::HiddenLine
            }),
            "display.edges" => self.set_look(if now == RenderMode::ShadedEdges {
                RenderMode::Shaded
            } else {
                RenderMode::ShadedEdges
            }),
            "display.refl" => match now {
                RenderMode::Realtime => self.set_look(RenderMode::RealtimeReflections),
                RenderMode::RealtimeReflections => self.set_look(RenderMode::Realtime),
                _ => {}
            },
            "display.overlays" => {
                let s = &mut self.shell.settings;
                s.render_overlays = !s.render_overlays;
                s.store();
                self.status = if s.render_overlays {
                    "Grids and dimensions show over the rendered view.".into()
                } else {
                    "Grids and dimensions are left off the rendered view.".into()
                };
            }
            _ => return false,
        }
        true
    }

    /// Whether grids, levels and dimensions are left off the 3D view: a
    /// Style or Rendered look shows, and they weren't asked for over it.
    pub(crate) fn overlays_off(&self) -> bool {
        let s = &self.shell.settings;
        s.render_3d.rendered().is_some() && !s.render_overlays
    }

    /// The Visuals tab: everything about how the 3D view looks, in the
    /// order it is worked on (Visuals REV02): Look, Light, Camera, Output.
    pub(crate) fn visuals_tab(&mut self, ui: &mut egui::Ui) {
        self.look_section(ui);
        // Light: the sun of the project's place, at a day and an hour
        let open = self.prop_header(ui, "visuals-light", "Light", true, Some("navigate"));
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| self.light_rows(ui));
        });
        // Camera: how bright the image is
        let open = self.prop_header(ui, "visuals-camera", "Camera", true, Some("navigate"));
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| self.camera_rows(ui));
        });
        // Output: the image saved
        let open = self.prop_header(ui, "visuals-output", "Output", true, Some("navigate"));
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| self.output_rows(ui));
        });
    }

    /// The moods: a named moment of light, each a time, a sky and a
    /// brightness that suit it (Visuals REV02, Light).
    pub(crate) const MOODS: [(&'static str, &'static str); 5] = [
        ("Day", "The middle of the day under a clear sky: high sun, short sharp shadows"),
        ("Morning", "An hour and a half after sunrise: a low sun from the east, long shadows"),
        ("Evening", "An hour before sunset: a low warm sun from the west, long shadows"),
        ("Overcast", "The middle of the day under cloud: soft grey light, no hard shadows"),
        ("Night", "An hour and a half after sunset: dark outside, the lamps light the rooms. It needs light fittings in the model"),
    ];

    /// What mood `i` sets at a latitude on a day: the hour, the sky, its
    /// strength and the exposure.
    pub(crate) fn mood(
        i: usize,
        latitude: f32,
        day: f32,
    ) -> (f32, creabim_render::sky::SkyKind, f32, f32) {
        use creabim_render::sky::SkyKind as K;
        let (rise, set) = creabim_render::sky::sun_hours(latitude, day).unwrap_or((6.0, 18.0));
        let quarter = |h: f32| ((h * 4.0).round() / 4.0).clamp(4.0, 22.0);
        match i {
            1 => (quarter(rise + 1.5), K::Clear, 1.0, 0.3),
            2 => (quarter(set - 1.0), K::Clear, 1.0, 0.5),
            3 => (12.0, K::Overcast, 1.0, 0.3),
            4 => (quarter(set + 1.5), K::Clear, 1.0, 7.0),
            _ => (12.0, K::Clear, 1.0, 0.0),
        }
    }

    fn light_rows(&mut self, ui: &mut egui::Ui) {
        let info = self.doc.model().project_info().map(|(_, p)| p.clone());
        // the mood: the one whose light the settings are now, or your own
        {
            let latitude = info
                .as_ref()
                .and_then(|p| p.latitude)
                .unwrap_or(super::DEFAULT_LATITUDE) as f32;
            let s = &self.shell.settings;
            let now = (
                s.render_hour,
                creabim_render::sky::SkyKind::of_key(&s.render_sky),
                s.render_sky_strength,
                s.render_ev,
            );
            let at = (0..Self::MOODS.len()).find(|i| {
                let m = Self::mood(*i, latitude, s.render_day);
                (m.0 - now.0).abs() < 0.01
                    && m.1 == now.1
                    && (m.2 - now.2).abs() < 0.005
                    && (m.3 - now.3).abs() < 0.05
            });
            let mut pick = at;
            crate::props::row(ui, "Mood", |ui| {
                egui::ComboBox::from_id_salt("light.mood")
                    .icon(crate::props::combo_icon)
                    .selected_text(at.map_or("Your own", |i| Self::MOODS[i].0))
                    .width(ui.available_width())
                    .truncate()
                    .show_ui(ui, |ui| {
                        for (i, (name, tip)) in Self::MOODS.iter().enumerate() {
                            ui.selectable_value(&mut pick, Some(i), *name).on_hover_text(*tip);
                        }
                    })
                    .response
                    .on_hover_text("A moment of light: it sets the time, the sky and the exposure below. Change any of them and the mood is your own");
            });
            if pick != at {
                if let Some(i) = pick {
                    let m = Self::mood(i, latitude, self.shell.settings.render_day);
                    let s = &mut self.shell.settings;
                    s.render_hour = m.0;
                    s.render_sky = m.1.key().into();
                    s.render_sky_strength = m.2;
                    s.render_ev = m.3;
                    s.store();
                    self.status = format!(
                        "Mood: {}. {} sky at {}, {:+.1} EV.",
                        Self::MOODS[i].0,
                        m.1.label(),
                        crate::rendered::hour_label(m.0),
                        m.3
                    );
                }
            }
        }
        // Sun, Sky and Lamps each fold on their own inside Light, as the
        // mockup has them (Visuals REV02; owner, 2026-11-20)
        let open = crate::props::sub_fold(ui, "visuals-light-sun", "Sun", true);
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| self.sun_rows(ui));
        });
        let open = crate::props::sub_fold(ui, "visuals-light-sky", "Sky", true);
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| self.sky_rows(ui));
        });
        let open = crate::props::sub_fold(ui, "visuals-light-lamps", "Lamps", true);
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| self.lamp_rows(ui));
        });
        if !self.shell.settings.render_3d.rendered().is_some() {
            crate::props::note(
                ui,
                "The sun and the sky light the Style and Rendered looks; lamps, the Rendered ones.",
            );
        }
    }

    /// Light, Sun: the place, the date and the time.
    fn sun_rows(&mut self, ui: &mut egui::Ui) {
        let info = self.doc.model().project_info().map(|(_, p)| p.clone());
        let place = match info.as_ref().and_then(|p| p.latitude) {
            Some(lat) => format!("{:.2}° {}", lat.abs(), if lat < 0.0 { "S" } else { "N" }),
            None => "Paris (not set)".to_string(),
        };
        crate::props::row(ui, "Place", |ui| {
            ui.label(place).on_hover_text(
                "The project's latitude and true north: Manage, Project information. With none set, Paris",
            )
        });
        let s = &self.shell.settings;
        let (mut day, mut hour) = (s.render_day, s.render_hour);
        crate::props::row(ui, "Date", |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(&mut day)
                    .range(1.0..=365.0)
                    .speed(1.0)
                    .custom_formatter(|d, _| crate::rendered::day_label(d as f32))
                    .custom_parser(|_| None),
            )
            .on_hover_text("The day of the year: drag to change it")
        });
        crate::props::row(ui, "Time", |ui| {
            crate::props::fill(
                ui,
                egui::Slider::new(&mut hour, 4.0..=22.0)
                    .step_by(0.25)
                    .custom_formatter(|h, _| crate::rendered::hour_label(h as f32)),
            )
            .on_hover_text("The hour, solar time, at the project's place")
        });
        let s = &mut self.shell.settings;
        if (day, hour) != (s.render_day, s.render_hour) {
            s.render_day = day.round();
            s.render_hour = hour;
            s.store();
        }
    }

    /// Light, Sky: what it is like and how strong.
    fn sky_rows(&mut self, ui: &mut egui::Ui) {
        // the sky: what it is like, and how strong its own light is
        let mut kind = creabim_render::sky::SkyKind::of_key(&self.shell.settings.render_sky);
        let was = kind;
        crate::props::row(ui, "Sky", |ui| {
            egui::ComboBox::from_id_salt("light.sky")
                .icon(crate::props::combo_icon)
                .selected_text(kind.label())
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    for (k, tip) in [
                        (
                            creabim_render::sky::SkyKind::Clear,
                            "A strong sun, sharp shadows, blue overhead",
                        ),
                        (
                            creabim_render::sky::SkyKind::Hazy,
                            "A weaker sun, soft-edged shadows, a pale sky",
                        ),
                        (
                            creabim_render::sky::SkyKind::Overcast,
                            "Cloud all over: grey light from everywhere, no hard shadows",
                        ),
                    ] {
                        ui.selectable_value(&mut kind, k, k.label())
                            .on_hover_text(tip);
                    }
                });
        });
        let mut percent = (self.shell.settings.render_sky_strength * 100.0).round();
        let percent_was = percent;
        crate::props::row(ui, "Strength", |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(&mut percent).range(0.0..=300.0).speed(1.0).suffix(" %"),
            )
            .on_hover_text("How strong the sky's own light is: the light in the shade and indoors. The sun is not changed")
        });
        if kind != was || percent != percent_was {
            let s = &mut self.shell.settings;
            s.render_sky = kind.key().into();
            s.render_sky_strength = percent / 100.0;
            s.store();
        }
    }

    /// Light, Lamps: the fittings' groups, each with its strength.
    fn lamp_rows(&mut self, ui: &mut egui::Ui) {
        // the lamps: the model's light fittings by group (the one named
        // on the fitting's type, else the type's name), each group's
        // strength (0 puts it off)
        let types = self.lamp_types();
        let mut set = self.shell.settings.render_lamps.clone();
        let was = set.clone();
        for (name, count) in &types {
            let mut percent = (set.get(name).copied().unwrap_or(1.0) * 100.0).round();
            crate::props::row(ui, name, |ui| {
                crate::props::fill(
                    ui,
                    egui::DragValue::new(&mut percent)
                        .range(0.0..=400.0)
                        .speed(1.0)
                        .custom_formatter(|v, _| if v <= 0.0 { "Off".to_string() } else { format!("{v:.0} %") }),
                )
                .on_hover_text(format!(
                    "{name}: {count} in the model. How strong they are in a render; 0 puts them off. What each gives (output, colour, beam, group) is on the fitting: select one, Properties. They light the Rendered looks"
                ))
            });
            if (percent - 100.0).abs() < 0.5 {
                set.remove(name);
            } else {
                set.insert(name.clone(), percent / 100.0);
            }
        }
        if set != was {
            let s = &mut self.shell.settings;
            s.render_lamps = set;
            s.store();
        }
        if types.is_empty() {
            crate::props::note(ui, "No lamps: the model has no light fittings. Place one from Model, Objects, Lighting.");
        }
    }

    fn camera_rows(&mut self, ui: &mut egui::Ui) {
        // the lens, as a focal length on a full-frame camera; changing it
        // keeps what is at the middle of the view the size it was
        if self.camera.ortho {
            crate::props::row(ui, "Lens", |ui| {
                ui.label("none: a parallel view")
                    .on_hover_text("A parallel view has no lens. Switch to perspective for one")
            });
        } else {
            let mut mm = self.camera.lens_mm();
            let was = mm;
            crate::props::row(ui, "Lens", |ui| {
                crate::props::fill(
                    ui,
                    egui::DragValue::new(&mut mm)
                        .range(12.0..=200.0)
                        .speed(0.25)
                        .custom_formatter(|v, _| {
                            format!("{v:.0} mm · {:.0}°", 2.0 * (12.0 / v).atan().to_degrees())
                        })
                        .custom_parser(|t| {
                            t.trim().split(|c: char| !(c.is_ascii_digit() || c == '.')).next()?.parse().ok()
                        }),
                )
                .on_hover_text("The lens, as on a full-frame camera, and the angle it sees top to bottom: 18 to 24 mm wide, for rooms; 28 to 35 mm, a house from the street; 50 mm and up, long. The camera moves so that the middle of the view stays the size it is")
            });
            if (mm - was).abs() > 1e-3 {
                self.camera.set_lens_mm(mm);
                // kept for the next time CreaBIM opens
                let s = &mut self.shell.settings;
                s.camera_lens = self.camera.lens_mm();
                s.store();
            }
            let mut straight = self.camera.straight;
            if crate::props::check(ui, "Verticals", &mut straight, "Straight")
                .on_hover_text("Keep walls' edges upright in the picture, as an architect's shift lens does, where a tilted camera would make them lean. Not when looking steeply up or down")
                .changed()
            {
                self.camera.straight = straight;
                let s = &mut self.shell.settings;
                s.camera_straight = straight;
                s.store();
            }
        }
        let mut ev = self.shell.settings.render_ev;
        crate::props::row(ui, "Exposure", |ui| {
            crate::props::fill(
                ui,
                egui::DragValue::new(&mut ev)
                    .range(-4.0..=10.0)
                    .speed(0.05)
                    .fixed_decimals(1)
                    .suffix(" EV"),
            )
            .on_hover_text(
                "Brightness, in stops: 0 outdoors, about +3 inside by day, about +7 by lamplight",
            )
        });
        // the white balance: what colour of light reads as white
        const WHITES: [(&str, f32, &str); 4] = [
            (
                "Daylight",
                6500.0,
                "Daylight reads as white; lamplight, warm and orange",
            ),
            (
                "Cloudy",
                7500.0,
                "A little warmer than Daylight, for a grey or a shaded scene",
            ),
            (
                "Warm",
                3500.0,
                "Between the two: rooms at dusk, lamps on and windows still light",
            ),
            (
                "Lamplight",
                2700.0,
                "Lamplight reads as white, so a room at night is not orange; daylight goes blue",
            ),
        ];
        let mut white = self.shell.settings.render_white;
        let white_was = white;
        let named = WHITES.iter().find(|w| (w.1 - white).abs() < 1.0);
        crate::props::row(ui, "Balance", |ui| {
            egui::ComboBox::from_id_salt("camera.white")
                .icon(crate::props::combo_icon)
                .selected_text(match named {
                    Some(w) => w.0,
                    None => "Your own",
                })
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    for (name, k, tip) in WHITES {
                        ui.selectable_value(&mut white, k, format!("{name}, {k:.0} K")).on_hover_text(tip);
                    }
                    if ui.selectable_label(named.is_none(), "Your own").clicked() && named.is_some() {
                        white = 5000.0;
                    }
                })
                .response
                .on_hover_text("White balance: the colour of light the picture is balanced for, in the Rendered looks. It changes colour, not brightness");
        });
        if named.is_none() {
            crate::props::row(ui, "", |ui| {
                crate::props::fill(
                    ui,
                    egui::DragValue::new(&mut white)
                        .range(2000.0..=12_000.0)
                        .speed(25.0)
                        .suffix(" K"),
                )
            });
        }
        if white != white_was {
            let s = &mut self.shell.settings;
            s.render_white = white.round();
            s.store();
        }
        let s = &mut self.shell.settings;
        let ev = (ev * 10.0).round() / 10.0;
        if ev != s.render_ev {
            s.render_ev = ev;
            s.store();
        }
    }

    fn output_rows(&mut self, ui: &mut egui::Ui) {
        // the size a render is made at to keep (Render crop REV00, 5)
        let mut size = self.shell.settings.render_size;
        let was = size;
        let named = Self::SIZES
            .iter()
            .find(|(_, s)| *s == size)
            .map(|(n, _)| *n);
        crate::props::row(ui, "Size", |ui| {
            egui::ComboBox::from_id_salt("output.size")
                .icon(crate::props::combo_icon)
                .selected_text(named.unwrap_or("Your own"))
                .width(ui.available_width())
                .truncate()
                .show_ui(ui, |ui| {
                    for (name, s) in Self::SIZES {
                        ui.selectable_value(&mut size, s, name);
                    }
                    if ui.selectable_label(named.is_none(), "Your own").clicked() && named.is_some()
                    {
                        size = [1600, 900];
                    }
                });
        });
        if named.is_none() {
            for (label, v) in [("Width", 0usize), ("Height", 1)] {
                crate::props::row(ui, label, |ui| {
                    crate::props::fill(
                        ui,
                        egui::DragValue::new(&mut size[v])
                            .range(64..=8000)
                            .suffix(" px"),
                    )
                });
            }
        }
        if size != was {
            let s = &mut self.shell.settings;
            s.render_size = size;
            s.store();
        }
        let own = size[0] == 0 || size[1] == 0;
        let shown = self
            .rendered
            .image
            .as_ref()
            .map_or("no image yet".to_string(), |i| {
                format!("{} × {} px", i.w, i.h)
            });
        if own {
            crate::props::row(ui, "Now", |ui| {
                ui.label(shown)
                    .on_hover_text("The 3D view as it is rendered now")
            });
        }
        let can = self.shell.settings.render_3d.rendered().is_some();
        if let Some(job) = &self.renders.making {
            let text = format!("{} × {}: {} · {:.0} s", job.size[0], job.size[1], job.stage, job.seconds);
            crate::props::row(ui, "Making", |ui| ui.label(text));
            if crate::props::action(ui, "", "Stop", "Stop this render; nothing is kept", true).clicked() {
                self.renders.making = None;
                self.status = "The render was stopped; nothing was kept.".into();
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        } else if crate::props::action(
            ui,
            "",
            if own { "Keep as a render" } else { "Render and keep" },
            if own {
                "Keep the 3D view as it is rendered now in the project's Renders"
            } else {
                "Render the 3D view at this size, in the background, and keep it in the project's Renders. Its shape may differ from the view's, so more or less shows at the sides"
            },
            can && (!own || self.rendered.image.is_some()),
        )
        .clicked()
        {
            let now = ui.input(|i| i.time);
            self.render_and_keep(now);
        }
        if crate::props::action(
            ui,
            "",
            "Save image…",
            "Save the 3D view as it is rendered now as a PNG, where you choose",
            self.rendered.image.is_some(),
        )
        .clicked()
        {
            self.save_rendered();
        }
    }

    /// The Look section: the style and the renderer the Style and
    /// Rendered buttons show.
    pub(crate) fn look_section(&mut self, ui: &mut egui::Ui) {
        // a folding block with its help, as the others in this tab (MU107)
        let open = self.prop_header(ui, "visuals-look", "Look", true, Some("navigate"));
        let mut done = false;
        crate::props::fold(ui, open, |ui| {
            ui.vertical(|ui| self.look_rows(ui));
            done = true;
        });
        let _ = done;
    }

    /// The lit display button's own rows, so the block follows the
    /// buttons (MU112: "I thought switching these did affect this?").
    fn look_rows(&mut self, ui: &mut egui::Ui) {
        let now = self.shell.settings.render_3d;
        let family = now.family();
        crate::props::row(ui, "Showing", |ui| {
            ui.label(family.label())
                .on_hover_text("The display button lit in the 3D view's header")
        });
        let mut pick = now;
        match family {
            Family::Line => {
                let mut hidden = now == RenderMode::HiddenLine;
                crate::props::check(ui, "Hidden", &mut hidden, "Hide the back")
                    .on_hover_text("White surfaces hide what is behind them. Off: wireframe");
                pick = if hidden {
                    RenderMode::HiddenLine
                } else {
                    RenderMode::Wireframe
                };
            }
            Family::Solid => {
                let mut edges = now == RenderMode::ShadedEdges;
                crate::props::check(ui, "Edges", &mut edges, "Drawn over")
                    .on_hover_text("Every edge drawn over the shaded surfaces");
                pick = if edges {
                    RenderMode::ShadedEdges
                } else {
                    RenderMode::Shaded
                };
            }
            Family::Style => {
                crate::props::row(ui, "Style", |ui| {
                    egui::ComboBox::from_id_salt("look.style")
                        .icon(crate::props::combo_icon)
                        .selected_text(pick.label())
                        .width(ui.available_width())
                        .truncate()
                        .show_ui(ui, |ui| {
                            for m in RenderMode::STYLES {
                                ui.selectable_value(&mut pick, m, m.label())
                                    .on_hover_text(m.hint());
                            }
                        });
                });
            }
            Family::Rendered => {
                let mut quick = now != RenderMode::PathTraced;
                let mut refl = now == RenderMode::RealtimeReflections;
                crate::props::row(ui, "Renderer", |ui| {
                    egui::ComboBox::from_id_salt("look.renderer")
                        .icon(crate::props::combo_icon)
                        .selected_text(if quick { "Quick" } else { "Photoreal" })
                        .width(ui.available_width())
                        .truncate()
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut quick, true, "Quick")
                                .on_hover_text(RenderMode::Realtime.hint());
                            ui.selectable_value(&mut quick, false, "Photoreal")
                                .on_hover_text(RenderMode::PathTraced.hint());
                        });
                });
                if quick {
                    crate::props::check(ui, "Reflect", &mut refl, "Glass, metal")
                        .on_hover_text("Glass and metal reflect the scene");
                }
                pick = match (quick, refl) {
                    (false, _) => RenderMode::PathTraced,
                    (true, true) => RenderMode::RealtimeReflections,
                    (true, false) => RenderMode::Realtime,
                };
            }
        }
        if pick != now {
            self.set_look(pick);
        }
        // grids and dimensions over a look that is rendered
        if now.rendered().is_some() {
            let mut overlays = self.shell.settings.render_overlays;
            if crate::props::check(ui, "Overlay", &mut overlays, "Grids, levels")
                .on_hover_text("Grid bubbles, levels and dimensions over a Style or Rendered look. Saved images leave them out")
                .changed()
            {
                self.display_action("display.overlays", "");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_mood_sets_a_time_a_sky_and_a_brightness_that_suit_it() {
        use creabim_render::sky::SkyKind as K;
        type A = crate::app::CreaBimApp;
        // Paris, midsummer: the sun sets at about 20:00 solar time
        let (rise, set) = creabim_render::sky::sun_hours(48.85, 172.0).unwrap();
        let day = A::mood(0, 48.85, 172.0);
        assert_eq!((day.0, day.1, day.3), (12.0, K::Clear, 0.0));
        let morning = A::mood(1, 48.85, 172.0);
        assert!((morning.0 - (rise + 1.5)).abs() <= 0.125 && morning.1 == K::Clear);
        let evening = A::mood(2, 48.85, 172.0);
        assert!(
            (evening.0 - (set - 1.0)).abs() <= 0.125,
            "{evening:?} {set}"
        );
        // earlier in winter, and always on a quarter of an hour
        let winter = A::mood(2, 48.85, 355.0);
        assert!(winter.0 < evening.0 - 3.0 && (winter.0 * 4.0).fract() == 0.0);
        let over = A::mood(3, 48.85, 172.0);
        assert_eq!((over.1, over.0), (K::Overcast, 12.0));
        assert!(over.3 > day.3, "cloud wants more exposure");
        // where the sun does not set, a mood still has an hour
        assert!((4.0..=22.0).contains(&A::mood(2, 80.0, 172.0).0));
        // night: after sunset, and far more exposure, for lamps
        let night = A::mood(4, 48.85, 172.0);
        assert!(night.0 > set + 1.0 && night.3 > 5.0);
        assert_eq!(A::MOODS.len(), 5);
    }

    use super::*;

    #[test]
    fn every_look_has_a_button_and_each_button_a_first_look() {
        for m in RenderMode::ALL {
            let f = m.family();
            assert_eq!(f.first().family(), f, "{m:?}");
            assert_eq!(Family::of_key(f.key()), Some(f));
        }
        // the first four are drawn every frame, the rest rendered
        for m in RenderMode::ALL {
            let fast = matches!(m.family(), Family::Line | Family::Solid);
            assert_eq!(m.rendered().is_none(), fast, "{m:?}");
        }
        assert!(RenderMode::STYLES
            .iter()
            .all(|m| m.family() == Family::Style));
    }

    #[test]
    fn a_button_shows_its_familys_last_look_and_the_caret_its_option() {
        let mut app = crate::app::test_app();
        // Solid with edges, then Line, then Solid again: edges kept
        app.set_look(RenderMode::ShadedEdges);
        assert!(app.display_action("display", "line"));
        assert_eq!(app.shell.settings.render_3d.family(), Family::Line);
        assert!(app.display_action("display", "solid"));
        assert_eq!(app.shell.settings.render_3d, RenderMode::ShadedEdges);
        // the caret lists Solid's two looks and no other, this one ticked
        let items = app.display_items();
        assert_eq!(items.len(), 2);
        assert!(items.iter().any(|i| matches!(i, Item::Action { label, checked: Some(true), .. } if label == "Shaded with edges")));
        app.display_action("display.edges", "");
        assert_eq!(app.shell.settings.render_3d, RenderMode::Shaded);
    }

    #[test]
    fn annotation_is_left_off_a_rendered_look_unless_asked() {
        let mut app = crate::app::test_app();
        app.set_look(RenderMode::ShadedEdges);
        assert!(!app.overlays_off(), "a solid look keeps its grids");
        app.set_look(RenderMode::Toon);
        assert!(app.overlays_off());
        app.display_action("display.overlays", "");
        assert!(!app.overlays_off());
        app.display_action("display.overlays", "");
        app.set_look(RenderMode::PathTraced);
        assert!(app.overlays_off());
    }
}

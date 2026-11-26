//! The keymap (first-run setup, decisions 2–4, owner 2026-11-16): the
//! keys of the edit commands by the program you come from, with your own
//! changes on top. A key can be a chord (M, Shift+D, Ctrl+E) or a short
//! sequence typed in a row (MV, RO), as Revit and AutoCAD do. Each frame
//! [`process`] turns the keys pressed into the commands they fire; the
//! handlers ask [`fired`]. A background other than CreaBIM's takes its
//! keys: what it uses no longer does what CreaBIM gave it (decision 4).

use eframe::egui;
use std::collections::BTreeMap;

/// The commands the keymap carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Act {
    /// Move by a base point, then where it goes.
    Move,
    /// Move with the pointer, straight away (Blender's G).
    Grab,
    Rotate,
    Scale,
    /// Copy, then move the copy.
    Copy,
    Mirror,
    Offset,
    /// Align's Corner: run two lines to where they meet (trim/extend).
    Corner,
    Pull,
    Stretch,
}

impl Act {
    pub const ALL: [Act; 10] = [
        Act::Move,
        Act::Grab,
        Act::Rotate,
        Act::Scale,
        Act::Copy,
        Act::Mirror,
        Act::Offset,
        Act::Corner,
        Act::Pull,
        Act::Stretch,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Act::Move => "move",
            Act::Grab => "grab",
            Act::Rotate => "rotate",
            Act::Scale => "scale",
            Act::Copy => "copy",
            Act::Mirror => "mirror",
            Act::Offset => "offset",
            Act::Corner => "corner",
            Act::Pull => "pull",
            Act::Stretch => "stretch",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Act::Move => "Move (base point)",
            Act::Grab => "Move with the pointer",
            Act::Rotate => "Rotate",
            Act::Scale => "Scale",
            Act::Copy => "Copy",
            Act::Mirror => "Mirror",
            Act::Offset => "Offset",
            Act::Corner => "Corner (trim / extend)",
            Act::Pull => "Pull",
            Act::Stretch => "Stretch",
        }
    }
}

/// A background's keys, by command; "" for none.
pub fn defaults(background: &str) -> BTreeMap<Act, &'static str> {
    use Act::*;
    let keys: &[(Act, &str)] = match background {
        "revit" => &[
            (Move, "MV"),
            (Rotate, "RO"),
            (Scale, "RE"),
            (Copy, "CO"),
            (Mirror, "MM"),
            (Offset, "OF"),
            (Corner, "TR"),
            (Pull, "P"),
            (Stretch, "Shift+S"),
        ],
        "archicad" => &[
            (Move, "Ctrl+D"),
            (Rotate, "Ctrl+E"),
            (Scale, "Ctrl+K"),
            (Copy, "Ctrl+Shift+D"),
            (Mirror, "Ctrl+M"),
            (Corner, "J"),
            (Pull, "P"),
            (Stretch, "Ctrl+H"),
        ],
        "autocad" => &[
            (Move, "M"),
            (Rotate, "RO"),
            (Scale, "SC"),
            (Copy, "CO"),
            (Mirror, "MI"),
            (Offset, "O"),
            (Corner, "TR"),
            (Pull, "P"),
            (Stretch, "Shift+S"),
        ],
        "sketchup" => &[
            (Move, "M"),
            (Rotate, "Q"),
            (Scale, "S"),
            (Copy, "Shift+D"),
            (Mirror, "Ctrl+M"),
            (Offset, "F"),
            (Corner, "J"),
            (Pull, "P"),
            (Stretch, "Shift+S"),
        ],
        // CreaBIM's own, and Blender's (CreaBIM's are Blender's)
        _ => &[
            (Move, "M"),
            (Grab, "G"),
            (Rotate, "R"),
            (Scale, "S"),
            (Copy, "Shift+D"),
            // Blender's and Archicad's (MU142)
            (Mirror, "Ctrl+M"),
            (Corner, "J"),
            (Pull, "P"),
            (Stretch, "Shift+S"),
        ],
    };
    let mut out: BTreeMap<Act, &str> = Act::ALL.iter().map(|a| (*a, "")).collect();
    for (a, k) in keys {
        out.insert(*a, k);
    }
    out
}

/// The mouse a background works with: "creabim" (middle orbits, right
/// pans) or "cad" (middle pans, Shift + middle orbits).
pub fn mouse_of(background: &str) -> &'static str {
    match background {
        "revit" | "archicad" | "autocad" => "cad",
        _ => "creabim",
    }
}

/// The keys in use: the background's, with the user's own on top.
pub fn bindings(background: &str, own: &BTreeMap<String, String>) -> Vec<(Act, String)> {
    defaults(background)
        .into_iter()
        .map(|(a, k)| (a, own.get(a.id()).cloned().unwrap_or_else(|| k.to_string())))
        .collect()
}

/// A key as written: a chord ("Shift+D", "Ctrl+E", "M") or a sequence of
/// letters ("MV").
#[derive(Clone, Debug, PartialEq)]
pub enum Keys {
    Chord(egui::Modifiers, egui::Key),
    Sequence(Vec<egui::Key>),
}

pub fn parse(text: &str) -> Option<Keys> {
    let t = text.trim();
    if t.is_empty() {
        return None;
    }
    if t.contains('+') || t.chars().count() == 1 {
        let mut m = egui::Modifiers::NONE;
        let mut key = None;
        for part in t.split('+').map(str::trim) {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "cmd" => m.command = true,
                "shift" => m.shift = true,
                "alt" => m.alt = true,
                k => key = egui::Key::from_name(&k.to_ascii_uppercase()),
            }
        }
        return key.map(|k| Keys::Chord(m, k));
    }
    let keys: Option<Vec<egui::Key>> = t
        .chars()
        .map(|c| egui::Key::from_name(&c.to_ascii_uppercase().to_string()))
        .collect();
    keys.filter(|k| k.len() > 1).map(Keys::Sequence)
}

#[derive(Clone, Default)]
struct Pending {
    keys: Vec<egui::Key>,
    at: f64,
}

fn fired_id() -> egui::Id {
    egui::Id::new("keymap.fired")
}

/// Read this frame's keys against `binds`: what they fire is kept for
/// [`fired`]. With a background other than CreaBIM's (`takes`), the keys
/// matched are taken, so what CreaBIM gave them does not run as well.
pub fn process(ctx: &egui::Context, binds: &[(Act, String)], takes: bool) {
    let mut fired: Vec<Act> = vec![];
    if ctx.egui_wants_keyboard_input() {
        ctx.data_mut(|d| d.insert_temp(fired_id(), fired));
        mark(ctx);
        return;
    }
    let parsed: Vec<(Act, Keys)> = binds
        .iter()
        .filter_map(|(a, k)| parse(k).map(|p| (*a, p)))
        .collect();
    // chords
    for (a, k) in &parsed {
        if let Keys::Chord(m, key) = k {
            let hit = ctx.input(|i| {
                i.key_pressed(*key)
                    && i.modifiers.command == m.command
                    && i.modifiers.shift == m.shift
                    && i.modifiers.alt == m.alt
            });
            if hit {
                fired.push(*a);
                if takes {
                    ctx.input_mut(|i| i.consume_key(*m, *key));
                }
            }
        }
    }
    // sequences: letters typed in a row, a second apart at most
    let seqs: Vec<(Act, &Vec<egui::Key>)> = parsed
        .iter()
        .filter_map(|(a, k)| match k {
            Keys::Sequence(s) => Some((*a, s)),
            _ => None,
        })
        .collect();
    if !seqs.is_empty() {
        let pid = egui::Id::new("keymap.pending");
        let now = ctx.input(|i| i.time);
        let mut pending: Pending = ctx.data(|d| d.get_temp(pid)).unwrap_or_default();
        if now - pending.at > 1.2 {
            pending.keys.clear();
        }
        let pressed: Vec<egui::Key> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        repeat: false,
                        modifiers,
                        ..
                    } if modifiers.is_none() => Some(*key),
                    _ => None,
                })
                .collect()
        });
        for key in pressed {
            let mut try_keys = pending.keys.clone();
            try_keys.push(key);
            let prefix = |ks: &Vec<egui::Key>| seqs.iter().any(|(_, s)| s.starts_with(ks));
            if !prefix(&try_keys) {
                // a fresh start with this key, or nothing
                try_keys = vec![key];
                if !prefix(&try_keys) {
                    pending.keys.clear();
                    continue;
                }
            }
            ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
            if let Some((a, _)) = seqs.iter().find(|(_, s)| **s == try_keys) {
                fired.push(*a);
                pending.keys.clear();
            } else {
                pending.keys = try_keys;
                pending.at = now;
            }
        }
        ctx.data_mut(|d| d.insert_temp(pid, pending));
    }
    ctx.data_mut(|d| d.insert_temp(fired_id(), fired));
    mark(ctx);
}

fn mark(ctx: &egui::Context) {
    let n = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("keymap.pass"), n));
}

/// Has the keymap read this frame's keys? (Not in tests that drive one
/// frame by hand: the handlers then read CreaBIM's own keys.)
pub fn active(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<u64>(egui::Id::new("keymap.pass"))) == Some(ctx.cumulative_pass_nr())
}

/// Did the keys pressed this frame fire `act`?
pub fn fired(ctx: &egui::Context, act: Act) -> bool {
    ctx.data(|d| d.get_temp::<Vec<Act>>(fired_id()))
        .is_some_and(|f| f.contains(&act))
}

/// The letters typed so far of a sequence, to show while waiting.
pub fn pending_text(ctx: &egui::Context) -> Option<String> {
    let p: Pending = ctx.data(|d| d.get_temp(egui::Id::new("keymap.pending")))?;
    (!p.keys.is_empty() && ctx.input(|i| i.time) - p.at <= 1.2)
        .then(|| p.keys.iter().map(|k| k.name()).collect())
}

/// Commands that share a key, for Settings to warn about.
pub fn clashes(binds: &[(Act, String)]) -> Vec<(Act, Act)> {
    let mut out = vec![];
    for (i, (a, ka)) in binds.iter().enumerate() {
        for (b, kb) in &binds[i + 1..] {
            if !ka.trim().is_empty() && ka.trim().eq_ignore_ascii_case(kb.trim()) {
                out.push((*a, *b));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CreaBIM's own keys, as the shortcuts page lists them: read through
    /// the keymap now, from its text.
    #[test]
    fn creabims_own_keys_are_the_shortcuts_pages() {
        let own = defaults("creabim");
        for (act, key) in [
            (Act::Move, egui::Key::M),
            (Act::Grab, egui::Key::G),
            (Act::Rotate, egui::Key::R),
            (Act::Scale, egui::Key::S),
            (Act::Corner, egui::Key::J),
            (Act::Pull, egui::Key::P),
        ] {
            assert_eq!(
                parse(own[&act]),
                Some(Keys::Chord(egui::Modifiers::NONE, key))
            );
        }
        assert_eq!(
            parse(own[&Act::Copy]),
            Some(Keys::Chord(egui::Modifiers::SHIFT, egui::Key::D))
        );
        // Mirror has its own key (MU142)
        assert_eq!(
            parse(own[&Act::Mirror]),
            Some(Keys::Chord(egui::Modifiers::COMMAND, egui::Key::M))
        );
    }

    #[test]
    fn keys_are_read_as_chords_or_sequences() {
        assert_eq!(
            parse("M"),
            Some(Keys::Chord(egui::Modifiers::NONE, egui::Key::M))
        );
        assert_eq!(
            parse("Shift+D"),
            Some(Keys::Chord(egui::Modifiers::SHIFT, egui::Key::D))
        );
        assert_eq!(
            parse("ctrl+e"),
            Some(Keys::Chord(egui::Modifiers::COMMAND, egui::Key::E))
        );
        assert_eq!(
            parse("MV"),
            Some(Keys::Sequence(vec![egui::Key::M, egui::Key::V]))
        );
        assert_eq!(parse(""), None);
    }

    #[test]
    fn a_background_brings_its_keys_and_your_own_win() {
        let mut own = BTreeMap::new();
        let revit = bindings("revit", &own);
        assert!(revit.contains(&(Act::Move, "MV".into())));
        assert!(revit.contains(&(Act::Rotate, "RO".into())));
        let creabim = bindings("creabim", &own);
        assert!(creabim.contains(&(Act::Grab, "G".into())));
        own.insert("move".into(), "Ctrl+Shift+M".into());
        assert!(bindings("revit", &own).contains(&(Act::Move, "Ctrl+Shift+M".into())));
        // the defaults clash nowhere
        for bg in [
            "creabim", "revit", "archicad", "autocad", "sketchup", "blender",
        ] {
            assert!(clashes(&bindings(bg, &BTreeMap::new())).is_empty(), "{bg}");
        }
        assert_eq!(mouse_of("revit"), "cad");
        assert_eq!(mouse_of("blender"), "creabim");
    }
}

//! CreaBIM Symbols: the owner's 150 drawing symbols (15 kinds, ten styles
//! each), and which one each kind of mark draws with (Mockup_Symbols_REV00,
//! A on all three, 2026-11-16).
//!
//! The library is `symbols/creabim-symbols.json`, built from the owner's
//! Archi-Glyph page by `tools/fonts/build_creabim_symbols.py`. A kind with no
//! pick draws CreaBIM's own mark, as it always has.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::{Model, View};

/// A stroke of a symbol: grid steps, y up.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SymbolStroke {
    pub pts: Vec<[f64; 2]>,
    /// 3 is a normal line; 2 finer, 4–6 heavier.
    pub weight: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SymbolFill {
    pub pts: Vec<[f64; 2]>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SymbolAnchor {
    pub at: [f64; 2],
}

/// Where a symbol's text goes (a grid's letter, a level's height).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SymbolText {
    pub at: [f64; 2],
    pub size: f64,
}

/// One style of a kind.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SymbolStyle {
    pub id: String,
    pub name: String,
    /// Technical, Modern, Conceptual or Hand.
    pub family: String,
    pub strokes: Vec<SymbolStroke>,
    pub fills: Vec<SymbolFill>,
    /// Rings: centre x, y and radius.
    pub circles: Vec<[f64; 3]>,
    pub anchor: SymbolAnchor,
    pub text: Option<SymbolText>,
}

/// A kind of mark and its ten styles.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SymbolKind {
    pub id: String,
    pub name: String,
    pub styles: Vec<SymbolStyle>,
}

#[derive(Deserialize)]
struct Library {
    kinds: Vec<SymbolKind>,
}

/// The shipped library, read once.
pub fn library() -> &'static [SymbolKind] {
    static LIB: OnceLock<Vec<SymbolKind>> = OnceLock::new();
    LIB.get_or_init(|| {
        let lib: Library = serde_json::from_str(include_str!("../symbols/creabim-symbols.json"))
            .expect("the shipped symbol library reads");
        lib.kinds
    })
}

/// A kind by its id (`grids`, `sections`…).
pub fn kind(id: &str) -> Option<&'static SymbolKind> {
    library().iter().find(|k| k.id == id)
}

/// A style by its id (`grids.classic_double_ring_axis`…).
pub fn style(id: &str) -> Option<&'static SymbolStyle> {
    let kind_id = id.split('.').next()?;
    kind(kind_id)?.styles.iter().find(|s| s.id == id)
}

/// Kinds that restyle a mark CreaBIM already draws, and the five placed
/// with the Symbol tool.
pub const MARK_KINDS: [&str; 10] = [
    "arrows",
    "sections",
    "elevations",
    "north",
    "callouts",
    "grids",
    "levels",
    "tags",
    "revisions",
    "scales",
];
pub const TOOL_KINDS: [&str; 5] = ["points", "slopes", "centerlines", "materials", "mep"];

/// Picks of a style pack or a view template: a style id for each kind
/// that has one.
pub type SymbolPicks = BTreeMap<String, String>;

/// The style kind `kind_id` draws with in view `v`: the view's template's
/// pick over the pack's; then, when the pack has that kind follow the line
/// style and the view draws Hand or Loose, the kind's first Hand-family
/// style. `None` is CreaBIM's own mark.
pub fn pick(model: &Model, v: &View, kind_id: &str) -> Option<&'static SymbolStyle> {
    let pack = crate::styles::active_pack(model);
    let r = crate::styles::resolved(model, v);
    if pack.symbols_follow.iter().any(|k| k == kind_id) {
        let ls = r
            .look
            .line_style
            .clone()
            .or_else(|| pack.line_style.clone());
        if matches!(ls.as_deref(), Some("Hand") | Some("Loose")) {
            if let Some(s) = kind(kind_id)?.styles.iter().find(|s| s.family == "Hand") {
                return Some(s);
            }
        }
    }
    let id = r
        .look
        .symbols
        .get(kind_id)
        .or_else(|| pack.symbols.get(kind_id))?;
    style(id)
}

/// The style a placed symbol draws with: its own, else its kind's pick
/// in view `v` (or the style pack's, on a sheet), else the kind's first.
pub fn placed_style(
    model: &Model,
    v: Option<&View>,
    p: &crate::PlacedSymbol,
) -> Option<&'static SymbolStyle> {
    if let Some(s) = p.style.as_deref().and_then(style) {
        return Some(s);
    }
    let picked = match v {
        Some(v) => pick(model, v, &p.kind),
        None => crate::styles::active_pack(model)
            .symbols
            .get(&p.kind)
            .and_then(|id| style(id)),
    };
    picked.or_else(|| kind(&p.kind)?.styles.first())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_library_has_fifteen_kinds_of_ten() {
        let lib = library();
        assert_eq!(lib.len(), 15);
        assert!(lib.iter().all(|k| k.styles.len() == 10));
        for k in MARK_KINDS.iter().chain(TOOL_KINDS.iter()) {
            assert!(kind(k).is_some(), "{k}");
        }
        let g = style("grids.classic_double_ring_axis").expect("a grid style");
        assert!(g.text.is_some());
    }

    #[test]
    fn a_pick_follows_template_pack_and_line_style() {
        let mut doc = crate::Document::from_model(crate::starter::starter_house());
        let view = doc
            .model()
            .iter()
            .find_map(|e| match &e.kind {
                crate::ElementKind::View(v) if matches!(v.kind, crate::ViewKind::Plan { .. }) => {
                    Some(v.clone())
                }
                _ => None,
            })
            .unwrap();
        assert!(pick(doc.model(), &view, "grids").is_none());
        let mut pack = crate::styles::active_pack(doc.model()).into_owned();
        pack.symbols
            .insert("grids".into(), "grids.hexagon_structural_node".into());
        doc.commit(crate::styles::apply_pack(doc.model(), &pack))
            .unwrap();
        assert_eq!(
            pick(doc.model(), &view, "grids").unwrap().name,
            "Hexagon Structural Node"
        );
        // following the line style: Hand takes the Hand family
        pack.symbols_follow.push("grids".into());
        pack.line_style = Some("Hand".into());
        doc.commit(crate::styles::apply_pack(doc.model(), &pack))
            .unwrap();
        assert_eq!(pick(doc.model(), &view, "grids").unwrap().family, "Hand");
    }
}

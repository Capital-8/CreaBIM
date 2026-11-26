//! How a door or window is made beyond its grid of cells: what its door
//! leaves are (flush, panelled, glazed and so on), the trim round it (a
//! lining on the reveal, an architrave on the room face, a threshold
//! under a door), and the ratings declared for it.
//!
//! Ratings are information only: CreaBIM records and reports what is
//! declared, it does not check or certify it.

use serde::{Deserialize, Serialize};

/// What a door's leaves are made as.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LeafStyle {
    /// A plain slab, the same both faces.
    #[default]
    Flush,
    /// Stiles and rails round four sunk panels.
    Panelled,
    /// Stiles and rails round one light of glass.
    Glazed,
    /// Glass in its upper part, a solid panel below.
    PartGlazed,
    /// Stiles and rails round horizontal slats.
    Louvred,
    /// A flush leaf with a narrow upright vision panel.
    VisionPanel,
    /// A frameless leaf of toughened glass, held by metal fittings
    /// ([`GlassFittings`]) and opened by a long pull handle.
    AllGlass,
    /// Stiles and rails round glass divided by glazing bars into small
    /// panes.
    French,
    /// Thin black steel stiles and rails round glass divided into panes.
    SlimSteel,
}

impl LeafStyle {
    pub const ALL: [LeafStyle; 9] = [
        LeafStyle::Flush,
        LeafStyle::Panelled,
        LeafStyle::Glazed,
        LeafStyle::PartGlazed,
        LeafStyle::Louvred,
        LeafStyle::VisionPanel,
        LeafStyle::AllGlass,
        LeafStyle::French,
        LeafStyle::SlimSteel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            LeafStyle::Flush => "Flush",
            LeafStyle::Panelled => "Panelled",
            LeafStyle::Glazed => "Glazed",
            LeafStyle::PartGlazed => "Part glazed",
            LeafStyle::Louvred => "Louvred",
            LeafStyle::VisionPanel => "Vision panel",
            LeafStyle::AllGlass => "All glass",
            LeafStyle::French => "French",
            LeafStyle::SlimSteel => "Slim steel",
        }
    }

    pub fn tip(self) -> &'static str {
        match self {
            LeafStyle::Flush => "A plain slab, the same on both faces.",
            LeafStyle::Panelled => "Stiles and rails round four sunk panels.",
            LeafStyle::Glazed => "Stiles and rails round one light of glass.",
            LeafStyle::PartGlazed => "Glass in the upper part, a solid panel below.",
            LeafStyle::Louvred => "Stiles and rails round horizontal slats, for air.",
            LeafStyle::VisionPanel => "A flush leaf with a narrow upright panel of glass.",
            LeafStyle::AllGlass => {
                "A frameless leaf of toughened glass, held by metal fittings, with a pull handle."
            }
            LeafStyle::French => "Stiles and rails round small panes between glazing bars.",
            LeafStyle::SlimSteel => "Thin black steel stiles and rails round small panes.",
        }
    }

    /// Whether its glass is divided into panes.
    pub fn has_panes(self) -> bool {
        matches!(self, LeafStyle::French | LeafStyle::SlimSteel)
    }

    /// How many panes a leaf `w` × `h` of this style has across and up
    /// when none is set: about 300 × 360 for French, 400 × 450 for slim
    /// steel, at least one each way.
    pub fn auto_panes(self, w: f64, h: f64) -> [u8; 2] {
        let (pw, ph) = match self {
            LeafStyle::SlimSteel => (0.4, 0.45),
            _ => (0.3, 0.36),
        };
        let n = |l: f64, p: f64| (l / p).round().clamp(1.0, 12.0) as u8;
        [n(w, pw), n(h, ph)]
    }
}

/// How an all glass leaf is held: the fitting types of glass door
/// hardware.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GlassFittings {
    /// Type F: a patch fitting at each corner.
    #[default]
    Patch,
    /// Type BP: a rail along the bottom, a patch fitting at the top.
    BottomRail,
    /// Type A/P: a rail along the top and the bottom.
    Rails,
}

impl GlassFittings {
    pub const ALL: [GlassFittings; 3] = [
        GlassFittings::Patch,
        GlassFittings::BottomRail,
        GlassFittings::Rails,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GlassFittings::Patch => "Patches",
            GlassFittings::BottomRail => "Bottom rail",
            GlassFittings::Rails => "Rails",
        }
    }

    /// The fitting type's usual code: F, BP or A/P.
    pub fn code(self) -> &'static str {
        match self {
            GlassFittings::Patch => "F",
            GlassFittings::BottomRail => "BP",
            GlassFittings::Rails => "A/P",
        }
    }

    pub fn tip(self) -> &'static str {
        match self {
            GlassFittings::Patch => "Type F: a patch fitting at each corner of the glass.",
            GlassFittings::BottomRail => "Type BP: a rail along the bottom, a patch at the top.",
            GlassFittings::Rails => "Type A/P: a rail along the top and along the bottom.",
        }
    }
}

/// The trim round an opening, sizes in metres; 0 leaves it out.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Trim {
    /// The thickness of a lining board on the reveal, from the frame to
    /// the room face of the wall.
    #[serde(skip_serializing_if = "is_zero")]
    pub lining: f64,
    /// The face width of an architrave round the opening on the room
    /// face of the wall.
    #[serde(skip_serializing_if = "is_zero")]
    pub architrave: f64,
    /// The height of a threshold under a door, across the wall.
    #[serde(skip_serializing_if = "is_zero")]
    pub threshold: f64,
}

impl Trim {
    pub fn is_none(&self) -> bool {
        *self == Trim::default()
    }

    /// Sizes that can be built: none negative, none past a third of a
    /// metre, an architrave at most half a metre wide.
    pub fn fitted(self) -> Trim {
        let f = |v: f64, most: f64| {
            if v.is_finite() {
                v.clamp(0.0, most)
            } else {
                0.0
            }
        };
        Trim {
            lining: f(self.lining, 0.05),
            architrave: f(self.architrave, 0.5),
            threshold: f(self.threshold, 0.1),
        }
    }
}

/// How an architrave stands proud of the wall.
pub const ARCHITRAVE_THICKNESS: f64 = 0.02;

/// Performance declared for a door or window: information only, carried
/// to schedules and IFC as given, never checked.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ratings {
    /// Fire resistance, as declared: "FD30", "EI 30", "60 min".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub fire: String,
    /// Sound reduction, as declared: "Rw 32 dB".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub acoustic: String,
    /// Security class, as declared: "RC2", "PAS 24".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub security: String,
    /// Thermal transmittance (U-value) in W/m²K, as declared.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub u_value: Option<f64>,
    /// Declared suitable for people with disabilities.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub accessible: bool,
    /// Load-bearing, as set; none: worked out (see [`crate::common`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_bearing: Option<bool>,
    /// External, as set for one element; none: worked out from the
    /// model (the outside on one side).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external: Option<bool>,
}

impl Ratings {
    pub fn is_none(&self) -> bool {
        *self == Ratings::default()
    }
}

/// Everything about how an opening is made beyond its frame and grid.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Construction {
    #[serde(skip_serializing_if = "is_flush")]
    pub leaf: LeafStyle,
    /// How an all glass leaf is held.
    #[serde(skip_serializing_if = "is_patch")]
    pub fittings: GlassFittings,
    /// The panes across and up a French or slim steel leaf; none: about
    /// as [`LeafStyle::auto_panes`] gives.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub panes: Option<[u8; 2]>,
    #[serde(skip_serializing_if = "Trim::is_none")]
    pub trim: Trim,
    #[serde(skip_serializing_if = "Ratings::is_none")]
    pub ratings: Ratings,
}

impl Construction {
    pub fn is_none(&self) -> bool {
        *self == Construction::default()
    }
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

fn is_flush(l: &LeafStyle) -> bool {
    *l == LeafStyle::Flush
}

fn is_patch(f: &GlassFittings) -> bool {
    *f == GlassFittings::Patch
}

impl Construction {
    /// About the panes across and up each leaf of a door `w` × `h` in a
    /// frame `frame` wide, with `leaves` leaves side by side: what its
    /// drawing shows when none is set, near enough to show in a row.
    pub fn panes_about(&self, w: f64, h: f64, frame: f64, leaves: usize) -> [u8; 2] {
        let (m, bottom) = if self.leaf == LeafStyle::SlimSteel {
            (0.04, 0.06)
        } else {
            (0.1, 0.2)
        };
        let lw = (w - 2.0 * frame) / leaves.max(1) as f64;
        self.panes_for(lw - 2.0 * m, h - frame - bottom - m)
    }

    /// The panes across and up a leaf `w` × `h`.
    pub fn panes_for(&self, w: f64, h: f64) -> [u8; 2] {
        self.panes
            .map(|[a, u]| [a.clamp(1, 12), u.clamp(1, 12)])
            .unwrap_or_else(|| self.leaf.auto_panes(w, h))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_declared_is_nothing_saved() {
        assert_eq!(
            serde_json::to_string(&Construction::default()).unwrap(),
            "{}"
        );
        let c = Construction {
            leaf: LeafStyle::PartGlazed,
            fittings: GlassFittings::Patch,
            panes: None,
            trim: Trim {
                architrave: 0.07,
                ..Trim::default()
            },
            ratings: Ratings {
                fire: "FD30".into(),
                ..Ratings::default()
            },
        };
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(
            j,
            r#"{"leaf":"part-glazed","trim":{"architrave":0.07},"ratings":{"fire":"FD30"}}"#
        );
        assert_eq!(serde_json::from_str::<Construction>(&j).unwrap(), c);
    }

    #[test]
    fn glass_leaves_save_their_fittings_and_panes() {
        let c = Construction {
            leaf: LeafStyle::AllGlass,
            fittings: GlassFittings::Rails,
            ..Construction::default()
        };
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(j, r#"{"leaf":"all-glass","fittings":"rails"}"#);
        assert_eq!(serde_json::from_str::<Construction>(&j).unwrap(), c);
        let f = Construction {
            leaf: LeafStyle::French,
            panes: Some([2, 5]),
            ..Construction::default()
        };
        let j = serde_json::to_string(&f).unwrap();
        assert_eq!(j, r#"{"leaf":"french","panes":[2,5]}"#);
        // a French leaf's panes are about 300 × 360 when none is set
        let auto = Construction { panes: None, ..f };
        assert_eq!(auto.panes_for(0.62, 1.85), [2, 5]);
    }

    #[test]
    fn trim_is_kept_buildable() {
        let t = Trim {
            lining: -1.0,
            architrave: 9.0,
            threshold: f64::NAN,
        }
        .fitted();
        assert_eq!(
            t,
            Trim {
                lining: 0.0,
                architrave: 0.5,
                threshold: 0.0
            }
        );
    }
}

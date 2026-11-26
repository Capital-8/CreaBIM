//! The drawing palette ("chroma", pens stage 7, Mockup_Pens_REV05, A on all
//! four): a few settings give a whole drawing its colours, after the
//! owner's PRISME and CHROMA colour tools.
//!
//! A palette works out seven colour **roles**: paper, ink (every line),
//! fill (what is cut: walls, slabs), surface (floors and tints), context
//! (trees, neighbours), accent (furniture, a second hue in Duo) and
//! shadow. Steps are perceptual (OKLCH), so an olive and a lavender
//! palette read equally strong. The **legibility rule** is always on: ink
//! keeps 4.5 : 1 against the paper and the fill 1.6 : 1, nudged just
//! enough and said so.
//!
//! A palette is kept in the style pack and picked per view or view
//! template, with a project default, as pen sets are. Colour is looked up
//! as a drawing is painted: nothing is rebuilt when a palette changes.

use serde::{Deserialize, Serialize};

/// How a palette colours a drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PaletteMode {
    /// Black on white, as CreaBIM always drew.
    #[default]
    Ink,
    /// One hue for everything.
    Mono,
    /// A hue, and an accent hue for furniture and planting.
    Duo,
    /// The materials' own colours (the Material drawing styles).
    Material,
}

impl PaletteMode {
    pub const ALL: [PaletteMode; 4] = [
        PaletteMode::Ink,
        PaletteMode::Mono,
        PaletteMode::Duo,
        PaletteMode::Material,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PaletteMode::Ink => "Ink",
            PaletteMode::Mono => "Mono",
            PaletteMode::Duo => "Duo",
            PaletteMode::Material => "Material",
        }
    }

    pub fn hint(self) -> &'static str {
        match self {
            PaletteMode::Ink => "Black on white, as drawings have always printed",
            PaletteMode::Mono => "One hue for paper, ink and fills",
            PaletteMode::Duo => "A hue, and an accent hue for furniture and planting",
            PaletteMode::Material => "The materials' own colours, lines in ink",
        }
    }

    /// Do the hue and strength settings mean anything?
    pub fn tuned(self) -> bool {
        matches!(self, PaletteMode::Mono | PaletteMode::Duo)
    }
}

/// A palette: its mode and five settings, each 0..100 but the hues.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Palette {
    pub name: String,
    pub mode: PaletteMode,
    /// The hue, in degrees.
    pub hue: f64,
    /// How dark (or, on dark paper, how light) the ink is, 0..100.
    pub ink: f64,
    /// How much the paper takes the hue, 0..100.
    pub paper: f64,
    /// How strong the cut fill is, 0..100.
    pub fill: f64,
    /// The accent's turn from the hue, in degrees (Duo).
    pub accent: f64,
    /// Light ink on dark paper (Blueprint).
    pub dark: bool,
}

impl Default for Palette {
    fn default() -> Self {
        Palette::ink()
    }
}

/// The palettes that ship with CreaBIM, in their order (decision 4).
pub const SHIPPED: [&str; 7] = [
    "Ink",
    "Sepia",
    "Olive",
    "Terracotta",
    "Blueprint",
    "Lavender and coral",
    "Material",
];

impl Palette {
    pub fn ink() -> Palette {
        Palette {
            name: "Ink".into(),
            mode: PaletteMode::Ink,
            hue: 115.0,
            ink: 80.0,
            paper: 45.0,
            fill: 70.0,
            accent: 110.0,
            dark: false,
        }
    }

    fn mono(name: &str, hue: f64, ink: f64, paper: f64, fill: f64) -> Palette {
        Palette {
            name: name.into(),
            mode: PaletteMode::Mono,
            hue,
            ink,
            paper,
            fill,
            ..Palette::ink()
        }
    }

    /// Is it plain ink, drawn as CreaBIM always drew?
    pub fn is_ink(&self) -> bool {
        self.mode == PaletteMode::Ink
    }
}

/// A shipped palette as it comes from the factory, by name.
pub fn shipped(name: &str) -> Option<Palette> {
    Some(match name {
        "Ink" => Palette::ink(),
        "Sepia" => Palette::mono("Sepia", 65.0, 78.0, 55.0, 62.0),
        "Olive" => Palette::mono("Olive", 115.0, 80.0, 45.0, 70.0),
        "Terracotta" => Palette::mono("Terracotta", 38.0, 74.0, 40.0, 78.0),
        "Blueprint" => Palette {
            dark: true,
            ..Palette::mono("Blueprint", 252.0, 86.0, 60.0, 55.0)
        },
        "Lavender and coral" => Palette {
            mode: PaletteMode::Duo,
            accent: 110.0,
            ..Palette::mono("Lavender and coral", 285.0, 70.0, 50.0, 55.0)
        },
        "Material" => Palette {
            name: "Material".into(),
            mode: PaletteMode::Material,
            ..Palette::ink()
        },
        _ => return None,
    })
}

/// What each shipped palette is for, as its row says it.
pub fn about(name: &str) -> &'static str {
    match name {
        "Ink" => "Black on white: permits and construction sets.",
        "Sepia" => "Warm brown on cream: concept boards, competition sheets.",
        "Olive" => "Green-grey: landscape and garden plans.",
        "Terracotta" => "Earth red: warm-climate and masonry projects.",
        "Blueprint" => "Light lines on deep blue: the classic print.",
        "Lavender and coral" => "Two hues: furniture and planting stand apart.",
        "Material" => "The materials' own colours: client presentations.",
        _ => "",
    }
}

/// sRGB, 0..1.
pub type Rgb = [f32; 3];

/// A palette worked out: its seven colours, and what the legibility rule
/// did.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Roles {
    pub mode: PaletteMode,
    pub dark: bool,
    pub paper: Rgb,
    pub ink: Rgb,
    pub fill: Rgb,
    pub surface: Rgb,
    pub context: Rgb,
    pub accent: Rgb,
    pub shadow: Rgb,
    /// The palette's hue and accent hue, degrees.
    pub hue: f64,
    pub accent_hue: f64,
    /// The selection's colour on this palette: orange, unless the palette
    /// sits near orange, then the opposite hue.
    pub selection: Rgb,
    pub selection_moved: bool,
    /// What the legibility rule changed, in words ("" when nothing).
    pub note: &'static str,
}

/// The selection's usual orange.
pub const ORANGE: Rgb = [0.902, 0.471, 0.118];

impl Palette {
    /// The palette's colours, the legibility rule applied.
    pub fn roles(&self) -> Roles {
        let gray = |v: f32| [v, v, v];
        match self.mode {
            PaletteMode::Ink => {
                return Roles {
                    mode: self.mode,
                    dark: false,
                    paper: gray(1.0),
                    ink: gray(0.1),
                    fill: gray(0.2),
                    surface: gray(1.0),
                    context: gray(0.82),
                    accent: gray(0.93),
                    shadow: gray(0.86),
                    hue: self.hue,
                    accent_hue: self.hue,
                    selection: ORANGE,
                    selection_moved: false,
                    note: "",
                }
            }
            PaletteMode::Material => {
                return Roles {
                    mode: self.mode,
                    dark: false,
                    paper: gray(1.0),
                    ink: gray(0.12),
                    fill: [0.62, 0.62, 0.6],
                    surface: [0.93, 0.87, 0.72],
                    context: [0.66, 0.8, 0.6],
                    accent: [0.85, 0.62, 0.42],
                    shadow: gray(0.84),
                    hue: self.hue,
                    accent_hue: self.hue,
                    selection: ORANGE,
                    selection_moved: false,
                    note: "",
                }
            }
            _ => {}
        }
        let d = self.dark;
        let h = self.hue;
        let pt = self.paper.clamp(0.0, 100.0) / 100.0;
        let fs = self.fill.clamp(0.0, 100.0) / 100.0;
        let is = self.ink.clamp(0.0, 100.0) / 100.0;
        let paper = oklch(
            if d { 0.27 } else { 0.975 - 0.02 * pt },
            0.006 + 0.03 * pt,
            h,
        );
        // the ink: as dark as asked, then darker (lighter on dark paper)
        // until it reads 4.5 : 1 on the paper
        let mut l = if d {
            0.62 + 0.34 * is
        } else {
            0.62 - 0.44 * is
        };
        let mut ink = oklch(l, 0.05, h);
        let mut n = 0;
        while contrast(ink, paper) < 4.5 && n < 60 {
            l += if d { 0.01 } else { -0.01 };
            ink = oklch(l, 0.05, h);
            n += 1;
        }
        // the fill: strong enough to stand off the paper
        let mut fl = if d { 0.46 + 0.3 * fs } else { 0.82 - 0.4 * fs };
        let fc = 0.05 + 0.07 * fs;
        let mut fill = oklch(fl, fc, h);
        let mut k = 0;
        while contrast(fill, paper) < 1.6 && k < 60 {
            fl += if d { 0.01 } else { -0.01 };
            fill = oklch(fl, fc, h);
            k += 1;
        }
        let duo = self.mode == PaletteMode::Duo;
        let ah = if duo { h + self.accent } else { h };
        let note = if n > 0 {
            if d {
                "Ink lightened to stay readable"
            } else {
                "Ink darkened to stay readable"
            }
        } else if k > 0 {
            "Fill adjusted to stand out from the paper"
        } else {
            ""
        };
        let mut r = Roles {
            mode: self.mode,
            dark: d,
            paper,
            ink,
            fill,
            surface: oklch(if d { 0.33 } else { 0.93 }, 0.02 + 0.02 * pt, h),
            context: oklch(if d { 0.42 } else { 0.84 }, 0.05, if duo { ah } else { h }),
            accent: oklch(
                if d { 0.66 } else { 0.76 },
                if duo { 0.13 } else { 0.06 },
                ah,
            ),
            shadow: oklch(if d { 0.2 } else { 0.86 }, 0.02, h),
            hue: h,
            accent_hue: ah,
            selection: ORANGE,
            selection_moved: false,
            note,
        };
        // the selection keeps its orange unless the palette is orange
        // itself, or the orange would sink into the fill or the paper
        let near = |x: f64| {
            let d = (x.rem_euclid(360.0) - 55.0).abs();
            d.min(360.0 - d) < 38.0
        };
        if near(h)
            || (duo && near(ah))
            || contrast(ORANGE, r.fill) < 1.5
            || contrast(ORANGE, r.paper) < 1.8
        {
            r.selection = oklch(if d { 0.78 } else { 0.6 }, 0.19, h + 180.0);
            r.selection_moved = true;
        }
        r
    }
}

impl Roles {
    /// A line of grey `g` (0 ink black .. 1 paper white) in the palette:
    /// along the ramp from the ink to the paper.
    pub fn line_grey(&self, g: f32) -> Rgb {
        match self.mode {
            PaletteMode::Ink => [g, g, g],
            _ => mix_ok(self.ink, self.paper, g.clamp(0.0, 1.0)),
        }
    }

    /// A fill of grey `g`: dark fills (poché) take the fill colour, light
    /// ones the surface, white the paper.
    pub fn fill_grey(&self, g: f32) -> Rgb {
        match self.mode {
            PaletteMode::Ink => [g, g, g],
            PaletteMode::Material => [g, g, g],
            _ => {
                let g = g.clamp(0.0, 1.0);
                if g >= 0.985 {
                    self.paper
                } else if g >= 0.6 {
                    // tints between the surface and the fill
                    mix_ok(self.fill, self.surface, (g - 0.6) / 0.385)
                } else {
                    // ink-dark fills (solid poché) stay a step darker
                    mix_ok(self.ink, self.fill, g / 0.6)
                }
            }
        }
    }

    /// A coloured mark or fill (a material colour, a tint, a tree) in the
    /// palette: its lightness kept, its hue the palette's (Mono) or the
    /// accent's (Duo). Ink and Material keep it.
    pub fn colour(&self, c: Rgb) -> Rgb {
        match self.mode {
            PaletteMode::Ink | PaletteMode::Material => c,
            _ => {
                let (l, ch, _) = to_oklch(c);
                let (hue, top) = if self.mode == PaletteMode::Duo {
                    (self.accent_hue, 0.13)
                } else {
                    (self.hue, 0.07)
                };
                // its lightness placed between the paper's and the ink's,
                // so a light tint stays light on dark paper too
                let (pl, il) = (to_oklch(self.paper).0, to_oklch(self.ink).0);
                let t = 1.0 - l;
                let lt = pl + (il - pl) * t;
                oklch(lt, ch.min(top).max(0.02), hue)
            }
        }
    }

    /// Ink's contrast on the paper, and the fill's.
    pub fn contrasts(&self) -> (f64, f64) {
        (
            contrast(self.ink, self.paper),
            contrast(self.fill, self.paper),
        )
    }
}

// ---- OKLCH ---------------------------------------------------------------

fn to_linear(v: f32) -> f64 {
    let v = f64::from(v);
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(x: f64) -> f32 {
    let x = x.clamp(0.0, 1.0);
    (if x <= 0.003_130_8 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }) as f32
}

/// OKLCH (lightness 0..1, chroma, hue in degrees) to sRGB.
pub fn oklch(l: f64, c: f64, h: f64) -> Rgb {
    let r = h.to_radians();
    let (a, b) = (c * r.cos(), c * r.sin());
    let l_ = (l + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
    let m_ = (l - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
    let s_ = (l - 0.089_484_177_5 * a - 1.291_485_548 * b).powi(3);
    [
        to_srgb(4.076_741_662_1 * l_ - 3.307_711_591_3 * m_ + 0.230_969_929_2 * s_),
        to_srgb(-1.268_438_004_6 * l_ + 2.609_757_401_1 * m_ - 0.341_319_396_5 * s_),
        to_srgb(-0.004_196_086_3 * l_ - 0.703_418_614_7 * m_ + 1.707_614_701 * s_),
    ]
}

/// sRGB to OKLab.
fn to_oklab(c: Rgb) -> (f64, f64, f64) {
    let (r, g, b) = (to_linear(c[0]), to_linear(c[1]), to_linear(c[2]));
    let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
    let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
    let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
    (
        0.210_454_255_3 * l + 0.793_617_785 * m - 0.004_072_046_8 * s,
        1.977_998_495_1 * l - 2.428_592_205 * m + 0.450_593_709_9 * s,
        0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766 * s,
    )
}

/// sRGB to OKLCH (lightness, chroma, hue in degrees).
pub fn to_oklch(c: Rgb) -> (f64, f64, f64) {
    let (l, a, b) = to_oklab(c);
    (
        l,
        (a * a + b * b).sqrt(),
        b.atan2(a).to_degrees().rem_euclid(360.0),
    )
}

/// `a` toward `b` by `t`, in OKLab, so the steps look even.
pub fn mix_ok(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let (al, aa, ab) = to_oklab(a);
    let (bl, ba, bb) = to_oklab(b);
    let t = f64::from(t);
    let (l, x, y) = (al + (bl - al) * t, aa + (ba - aa) * t, ab + (bb - ab) * t);
    let (c, h) = ((x * x + y * y).sqrt(), y.atan2(x).to_degrees());
    oklch(l, c, h)
}

/// WCAG relative luminance.
fn luminance(c: Rgb) -> f64 {
    0.2126 * to_linear(c[0]) + 0.7152 * to_linear(c[1]) + 0.0722 * to_linear(c[2])
}

/// WCAG contrast ratio, 1..21.
pub fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (x, y) = (luminance(a), luminance(b));
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// How a palette looks to someone with a colour-vision difference (the
/// preview, as PRISME has): the common deuteranopia and protanopia, and
/// tritanopia, by Machado et al.'s matrices at full severity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Vision {
    #[default]
    Typical,
    Deuteranopia,
    Protanopia,
    Tritanopia,
    Greyscale,
}

impl Vision {
    pub const ALL: [Vision; 5] = [
        Vision::Typical,
        Vision::Deuteranopia,
        Vision::Protanopia,
        Vision::Tritanopia,
        Vision::Greyscale,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Vision::Typical => "Typical",
            Vision::Deuteranopia => "Deuteranopia",
            Vision::Protanopia => "Protanopia",
            Vision::Tritanopia => "Tritanopia",
            Vision::Greyscale => "Greyscale print",
        }
    }

    /// `c` as it would be seen.
    pub fn see(self, c: Rgb) -> Rgb {
        let m: [[f64; 3]; 3] = match self {
            Vision::Typical => return c,
            Vision::Greyscale => {
                let l = to_oklab(c).0;
                let g = oklch(l, 0.0, 0.0);
                return g;
            }
            Vision::Deuteranopia => [
                [0.367_322, 0.860_646, -0.227_968],
                [0.280_085, 0.672_501, 0.047_413],
                [-0.011_820, 0.042_940, 0.968_881],
            ],
            Vision::Protanopia => [
                [0.152_286, 1.052_583, -0.204_868],
                [0.114_503, 0.786_281, 0.099_216],
                [-0.003_882, -0.048_116, 1.051_998],
            ],
            Vision::Tritanopia => [
                [1.255_528, -0.076_749, -0.178_779],
                [-0.078_411, 0.930_809, 0.147_602],
                [0.004_733, 0.691_367, 0.303_900],
            ],
        };
        let v = [to_linear(c[0]), to_linear(c[1]), to_linear(c[2])];
        let mut out = [0.0f32; 3];
        for (i, row) in m.iter().enumerate() {
            out[i] = to_srgb(row[0] * v[0] + row[1] * v[1] + row[2] * v[2]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_palette_keeps_its_ink_and_fill_readable() {
        for name in SHIPPED {
            let p = shipped(name).unwrap();
            let r = p.roles();
            let (ink, fill) = r.contrasts();
            assert!(ink >= 4.5, "{name}: ink {ink}");
            assert!(fill >= 1.5, "{name}: fill {fill}");
        }
    }

    #[test]
    fn faint_ink_is_darkened_and_said_so() {
        let mut p = shipped("Olive").unwrap();
        p.ink = 0.0;
        p.paper = 100.0;
        let r = p.roles();
        assert!(r.contrasts().0 >= 4.5);
        assert!(r.note.contains("darkened"), "{}", r.note);
        // on dark paper the ink is lightened instead
        let mut b = shipped("Blueprint").unwrap();
        b.ink = 0.0;
        let r = b.roles();
        assert!(r.contrasts().0 >= 4.5);
        assert!(r.note.contains("lightened"), "{}", r.note);
    }

    #[test]
    fn the_selection_turns_away_from_an_orange_palette() {
        assert!(!shipped("Olive").unwrap().roles().selection_moved);
        let t = shipped("Terracotta").unwrap().roles();
        assert!(t.selection_moved);
        assert_ne!(t.selection, ORANGE);
    }

    #[test]
    fn oklch_round_trips_and_ink_draws_as_before() {
        let c = oklch(0.6, 0.1, 140.0);
        let (l, ch, h) = to_oklch(c);
        assert!((l - 0.6).abs() < 0.01 && (ch - 0.1).abs() < 0.01 && (h - 140.0).abs() < 1.0);
        let ink = Palette::ink().roles();
        assert_eq!(ink.line_grey(0.3), [0.3, 0.3, 0.3]);
        assert_eq!(ink.colour([0.2, 0.5, 0.9]), [0.2, 0.5, 0.9]);
        // Mono gives a grey line the palette's hue
        let olive = shipped("Olive").unwrap().roles();
        let (_, c, h) = to_oklch(olive.line_grey(0.0));
        assert!(c > 0.02 && (h - 115.0).abs() < 15.0, "{c} {h}");
    }

    #[test]
    fn colour_vision_and_greyscale_previews_change_colour_not_greys() {
        let grey = [0.5, 0.5, 0.5];
        for v in Vision::ALL {
            let g = v.see(grey);
            assert!((g[0] - g[2]).abs() < 0.03, "{v:?} {g:?}");
        }
        let red = [0.9, 0.2, 0.2];
        assert_ne!(Vision::Deuteranopia.see(red), red);
    }
}

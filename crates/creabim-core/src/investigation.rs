//! Ground investigation: what a soils report found at each spot it looked.
//! A borehole, a trial pit or a probe records the layers it went through,
//! by depth from its own ground level as the report gives them, the water
//! it met, and the tests made on the way down. These are observations,
//! entered as the report states them: CreaBIM draws them (on plan, in 3D
//! and as a borehole log) and never works anything out of them that the
//! engineer didn't write.

use glam::DVec2;
use serde::{Deserialize, Serialize};

/// How a spot was investigated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProbeKind {
    /// A hole drilled or bored, logged from its samples.
    #[default]
    Borehole,
    /// A pit dug by machine or hand and logged from its sides.
    TrialPit,
    /// A probe driven or pushed in (dynamic probe, cone): tests, few or no
    /// samples.
    Probe,
}

impl ProbeKind {
    pub const ALL: [ProbeKind; 3] = [ProbeKind::Borehole, ProbeKind::TrialPit, ProbeKind::Probe];

    pub fn label(self) -> &'static str {
        match self {
            ProbeKind::Borehole => "Borehole",
            ProbeKind::TrialPit => "Trial pit",
            ProbeKind::Probe => "Probe",
        }
    }

    /// The letters it is numbered with: BH1, TP1, DP1.
    pub fn letters(self) -> &'static str {
        match self {
            ProbeKind::Borehole => "BH",
            ProbeKind::TrialPit => "TP",
            ProbeKind::Probe => "DP",
        }
    }

    /// How deep a new one goes.
    pub fn usual_depth(self) -> f64 {
        match self {
            ProbeKind::Borehole => 6.0,
            ProbeKind::TrialPit => 2.5,
            ProbeKind::Probe => 4.0,
        }
    }
}

/// What a layer is made of, in the broad families reports draw with
/// their own symbols. The report's own words go in the layer's
/// description.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SoilKind {
    Topsoil,
    /// Fill placed by people: rubble, old foundations, imported soil.
    MadeGround,
    #[default]
    Clay,
    Silt,
    Sand,
    Gravel,
    /// Peat and other organic soils.
    Peat,
    /// Rock of any kind, weathered or sound.
    Rock,
}

impl SoilKind {
    pub const ALL: [SoilKind; 8] = [
        SoilKind::Topsoil,
        SoilKind::MadeGround,
        SoilKind::Clay,
        SoilKind::Silt,
        SoilKind::Sand,
        SoilKind::Gravel,
        SoilKind::Peat,
        SoilKind::Rock,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SoilKind::Topsoil => "Topsoil",
            SoilKind::MadeGround => "Made ground",
            SoilKind::Clay => "Clay",
            SoilKind::Silt => "Silt",
            SoilKind::Sand => "Sand",
            SoilKind::Gravel => "Gravel",
            SoilKind::Peat => "Peat",
            SoilKind::Rock => "Rock",
        }
    }

    /// The colour behind its symbol, sRGB 0..1: the earthy tints reports
    /// use.
    pub fn color(self) -> [f32; 3] {
        let c = |r: u8, g: u8, b: u8| [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0];
        match self {
            SoilKind::Topsoil => c(143, 130, 86),
            SoilKind::MadeGround => c(199, 160, 140),
            SoilKind::Clay => c(207, 154, 106),
            SoilKind::Silt => c(217, 195, 154),
            SoilKind::Sand => c(236, 212, 134),
            SoilKind::Gravel => c(200, 193, 168),
            SoilKind::Peat => c(110, 92, 72),
            SoilKind::Rock => c(174, 179, 184),
        }
    }

    /// The family a report's description names, by its main word: the
    /// word in capitals by the usual convention ("Firm brown silty CLAY"),
    /// else the last soil word in it. None when no soil word is there.
    pub fn from_description(text: &str) -> Option<SoilKind> {
        let words: Vec<&str> = text
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let of = |w: &str| -> Option<SoilKind> {
            let w = w.to_ascii_lowercase();
            Some(match w.as_str() {
                "topsoil" => SoilKind::Topsoil,
                "made" | "fill" | "rubble" => SoilKind::MadeGround,
                "clay" | "clays" => SoilKind::Clay,
                "silt" | "silts" => SoilKind::Silt,
                "sand" | "sands" => SoilKind::Sand,
                "gravel" | "gravels" | "cobbles" => SoilKind::Gravel,
                "peat" | "organic" => SoilKind::Peat,
                "rock" | "sandstone" | "mudstone" | "siltstone" | "limestone" | "chalk"
                | "granite" | "shale" | "dolomite" | "marl" | "basalt" | "laterite" | "slate"
                | "gneiss" | "schist" => SoilKind::Rock,
                _ => return None,
            })
        };
        // "MADE GROUND" and "TOPSOIL" lead the line in reports
        if let Some(k) = words.first().and_then(|w| of(w)) {
            if matches!(k, SoilKind::Topsoil | SoilKind::MadeGround) {
                return Some(k);
            }
        }
        words
            .iter()
            .filter(|w| w.len() > 2 && w.chars().all(|c| !c.is_lowercase()))
            .find_map(|w| of(w))
            .or_else(|| words.iter().rev().find_map(|w| of(w)))
    }
}

/// One layer of a log: from the foot of the one above (or the ground)
/// down to `to`, a depth below the investigation's ground level.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LogLayer {
    /// The depth of its foot.
    pub to: f64,
    #[serde(default)]
    pub soil: SoilKind,
    /// The report's name for the layer it belongs to across the site
    /// (GT1, Stratum B, Unit 3), if it gives one.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
    /// The report's description, as written.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

/// Water met on the way down.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WaterStrike {
    /// Where it was struck.
    pub depth: f64,
    /// Where it stood after waiting, if the report says.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rest: Option<f64>,
    /// Anything else the report says ("after 20 min", "seepage").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

/// A test made at a depth.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestKind {
    /// Standard penetration test: blows for 300 mm, N.
    #[default]
    Spt,
    /// Hand shear vane: undrained shear strength, kPa.
    Vane,
    /// Pocket penetrometer, kPa.
    Penetrometer,
    /// Dynamic probe: blows for 100 mm.
    Dynamic,
}

impl TestKind {
    pub const ALL: [TestKind; 4] = [
        TestKind::Spt,
        TestKind::Vane,
        TestKind::Penetrometer,
        TestKind::Dynamic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TestKind::Spt => "SPT N",
            TestKind::Vane => "Shear vane",
            TestKind::Penetrometer => "Penetrometer",
            TestKind::Dynamic => "Dynamic probe",
        }
    }

    /// What its value is measured in.
    pub fn unit(self) -> &'static str {
        match self {
            TestKind::Spt => "blows/300 mm",
            TestKind::Vane | TestKind::Penetrometer => "kPa",
            TestKind::Dynamic => "blows/100 mm",
        }
    }
}

/// A test at a depth and its result.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct FieldTest {
    pub depth: f64,
    #[serde(default)]
    pub kind: TestKind,
    pub value: f64,
    /// The result as the report writes it, when the number alone doesn't
    /// say it all ("50/220 mm" for a test stopped short).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
}

/// A borehole, trial pit or probe at `position` in plan.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Probe {
    pub position: DVec2,
    #[serde(default)]
    pub kind: ProbeKind,
    /// Its ground level as the report gives it, as an elevation like a
    /// level's; none takes the existing ground's height there.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ground: Option<f64>,
    /// How deep it went.
    pub depth: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<LogLayer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub water: Vec<WaterStrike>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tests: Vec<FieldTest>,
    /// How it was made ("Cable percussion, 150 mm").
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,
    /// When, as the report writes it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub date: String,
    /// The report it comes from (its reference, the firm).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub report: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub notes: String,
}

impl Probe {
    /// A new one of `kind` at `position`, its usual depth, with no layers
    /// described yet.
    pub fn new(position: DVec2, kind: ProbeKind) -> Self {
        Self {
            position,
            kind,
            ground: None,
            depth: kind.usual_depth(),
            layers: vec![],
            water: vec![],
            tests: vec![],
            method: String::new(),
            date: String::new(),
            report: String::new(),
            notes: String::new(),
        }
    }

    /// Each layer with the depth of its top: (top, layer).
    pub fn spans(&self) -> Vec<(f64, &LogLayer)> {
        let mut top = 0.0;
        self.layers
            .iter()
            .map(|l| {
                let t = top;
                top = l.to;
                (t, l)
            })
            .collect()
    }

    /// The layer at `depth` below its ground, if described there.
    pub fn layer_at(&self, depth: f64) -> Option<&LogLayer> {
        self.spans()
            .into_iter()
            .find(|(top, l)| depth >= *top - 1e-9 && depth <= l.to + 1e-9)
            .map(|(_, l)| l)
    }

    /// The shallowest water struck: (struck, rest), depths.
    pub fn water_level(&self) -> Option<(f64, Option<f64>)> {
        self.water
            .iter()
            .min_by(|a, b| a.depth.total_cmp(&b.depth))
            .map(|w| (w.depth, w.rest))
    }

    /// Put the layers in order and run the last one to the bottom when it
    /// stops short; a layer below the bottom deepens the hole to it.
    pub fn tidy(&mut self) {
        self.layers.sort_by(|a, b| a.to.total_cmp(&b.to));
        self.layers.dedup_by(|a, b| (a.to - b.to).abs() < 1e-6);
        if let Some(last) = self.layers.last() {
            self.depth = self.depth.max(last.to);
        }
        self.tests.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        self.water.sort_by(|a, b| a.depth.total_cmp(&b.depth));
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(self.position.is_finite()) {
            return Err("an investigation point needs a finite position".into());
        }
        if !(self.depth.is_finite() && self.depth > 0.0) {
            return Err(format!("its depth must be positive, got {}", self.depth));
        }
        if let Some(g) = self.ground {
            if !g.is_finite() {
                return Err("its ground level must be a number".into());
            }
        }
        let mut top = 0.0;
        for l in &self.layers {
            if !(l.to.is_finite() && l.to > top) {
                return Err(format!(
                    "layers must go down in order: one ends at {} below one ending at {top}",
                    l.to
                ));
            }
            top = l.to;
        }
        for w in &self.water {
            if !(w.depth.is_finite() && w.depth >= 0.0) {
                return Err("a water strike needs a depth".into());
            }
        }
        for t in &self.tests {
            if !(t.depth.is_finite() && t.depth >= 0.0 && t.value.is_finite()) {
                return Err("a test needs a depth and a value".into());
            }
        }
        Ok(())
    }

    /// Read layers from rows copied out of a report: each line a depth or
    /// a "from to" pair of depths, then the description. The soil is the
    /// one the description names; a code such as GT3 at the start of the
    /// text is the layer's unit. Returns how many rows were read.
    pub fn read_layers(&mut self, text: &str) -> usize {
        let mut read = vec![];
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut rest = line;
            let mut depths = vec![];
            while depths.len() < 2 {
                let rest_t = rest.trim_start();
                let end = rest_t
                    .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == ','))
                    .unwrap_or(rest_t.len());
                let num = &rest_t[..end];
                match num.replace(',', ".").parse::<f64>() {
                    Ok(v) if !num.is_empty() => {
                        depths.push(v);
                        rest = rest_t[end..].trim_start_matches(|c: char| {
                            c.is_whitespace() || c == '-' || c == '–' || c == 'm' || c == '\t'
                        });
                    }
                    _ => break,
                }
            }
            let Some(&to) = depths.last() else {
                continue;
            };
            let mut description = rest.trim().to_string();
            let mut unit = String::new();
            if let Some(first) = description.split_whitespace().next() {
                let code = first.trim_end_matches([':', '-', '.']);
                let looks_code = code.len() <= 6
                    && code.chars().any(|c| c.is_ascii_digit())
                    && code.chars().all(|c| c.is_ascii_alphanumeric());
                if looks_code {
                    unit = code.to_string();
                    description = description[first.len()..].trim().to_string();
                }
            }
            let soil = SoilKind::from_description(&description).unwrap_or_default();
            read.push(LogLayer {
                to,
                soil,
                unit,
                description,
            });
        }
        let n = read.len();
        if n > 0 {
            self.layers = read;
            self.tidy();
        }
        n
    }
}

/// The name a new investigation point of `kind` takes: the next number
/// after those of its kind already in `names`.
pub fn next_name<'a>(kind: ProbeKind, names: impl Iterator<Item = &'a str>) -> String {
    let letters = kind.letters();
    let most = names
        .filter_map(|n| n.strip_prefix(letters))
        .filter_map(|n| n.trim().parse::<u32>().ok())
        .max()
        .unwrap_or(0);
    format!("{letters}{}", most + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_from_a_report_become_layers_with_their_soil_and_unit() {
        let mut p = Probe::new(DVec2::ZERO, ProbeKind::Borehole);
        let n = p.read_layers(
            "0.00 0.25 GT1 TOPSOIL: dark brown sandy loam\n\
             0.25-1.05 GT2 MADE GROUND: brown sandy gravel with brick\n\
             1.05 – 3.05 GT3 Firm brown mottled grey silty CLAY\n\
             3.05\t5.25\tDense orange-brown fine to medium SAND\n\
             10,00 Weathered SANDSTONE",
        );
        assert_eq!(n, 5);
        let kinds: Vec<SoilKind> = p.layers.iter().map(|l| l.soil).collect();
        assert_eq!(
            kinds,
            vec![
                SoilKind::Topsoil,
                SoilKind::MadeGround,
                SoilKind::Clay,
                SoilKind::Sand,
                SoilKind::Rock
            ]
        );
        assert_eq!(p.layers[2].unit, "GT3");
        assert_eq!(
            p.layers[2].description,
            "Firm brown mottled grey silty CLAY"
        );
        assert!((p.layers[4].to - 10.0).abs() < 1e-9);
        assert!(
            (p.depth - 10.0).abs() < 1e-9,
            "the hole deepens to its last layer"
        );
        assert_eq!(p.layer_at(2.0).map(|l| l.soil), Some(SoilKind::Clay));
        assert!(p.validate().is_ok());
    }

    #[test]
    fn the_capitalised_word_names_the_soil() {
        assert_eq!(
            SoilKind::from_description("Firm brown sandy CLAY with gravel"),
            Some(SoilKind::Clay)
        );
        assert_eq!(
            SoilKind::from_description("Dense clayey SAND"),
            Some(SoilKind::Sand)
        );
        assert_eq!(
            SoilKind::from_description("soft grey silt"),
            Some(SoilKind::Silt)
        );
        assert_eq!(SoilKind::from_description("no description"), None);
    }

    #[test]
    fn names_count_on_from_those_of_their_kind() {
        let names = ["BH1", "BH3", "TP1", "Wall"];
        assert_eq!(next_name(ProbeKind::Borehole, names.into_iter()), "BH4");
        assert_eq!(next_name(ProbeKind::TrialPit, names.into_iter()), "TP2");
        assert_eq!(next_name(ProbeKind::Probe, names.into_iter()), "DP1");
    }

    #[test]
    fn layers_out_of_order_are_refused() {
        let mut p = Probe::new(DVec2::ZERO, ProbeKind::Borehole);
        p.layers = vec![
            LogLayer {
                to: 2.0,
                ..Default::default()
            },
            LogLayer {
                to: 1.0,
                ..Default::default()
            },
        ];
        assert!(p.validate().is_err());
        p.tidy();
        assert!(p.validate().is_ok());
    }
}

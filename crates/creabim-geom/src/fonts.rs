//! The fonts CreaBIM ships (its own and the Classics, 2026-11-16): their
//! files, their true widths for laying text out, and what an SVG or a PDF
//! needs to carry them. Helvetica, Times and Courier stay the PDF's own.

use creabim_core::Font;
use std::sync::OnceLock;

macro_rules! font_file {
    ($dir:literal, $file:literal) => {
        include_bytes!(concat!("../../creabim-app/assets/fonts/", $dir, "/", $file))
    };
}

/// The file of a shipped font; none for the PDF's own three.
pub fn bytes(f: Font) -> Option<&'static [u8]> {
    Some(match f {
        Font::Sans | Font::Serif | Font::Mono => return None,
        Font::Structa => font_file!("creabim", "CreaBIM-Structa-Regular.ttf"),
        Font::Vectra => font_file!("creabim", "CreaBIM-Vectra-Regular.ttf"),
        Font::Spencer => font_file!("creabim", "CreaBIM-Spencer-Regular.ttf"),
        Font::Chisel => font_file!("creabim", "CreaBIM-Chisel-Regular.ttf"),
        Font::Kinetic => font_file!("creabim", "CreaBIM-Kinetic-Regular.ttf"),
        Font::TechHand => font_file!("creabim", "CreaBIM-TechHand-Regular.ttf"),
        Font::AtelierHand => font_file!("creabim", "CreaBIM-AtelierHand-Regular.ttf"),
        Font::Inter => font_file!("classics", "Inter-Regular.ttf"),
        Font::NotoSans => font_file!("classics", "NotoSans-Regular.ttf"),
        Font::IbmPlexSans => font_file!("classics", "IBMPlexSans-Regular.ttf"),
        Font::IbmPlexSansCondensed => font_file!("classics", "IBMPlexSansCondensed-Regular.ttf"),
        Font::IbmPlexMono => font_file!("classics", "IBMPlexMono-Regular.ttf"),
        Font::ArchivoNarrow => font_file!("classics", "ArchivoNarrow-Regular.ttf"),
        Font::BarlowCondensed => font_file!("classics", "BarlowCondensed-Regular.ttf"),
        Font::Jost => font_file!("classics", "Jost-Regular.ttf"),
        Font::Ubuntu => font_file!("classics", "Ubuntu-Regular.ttf"),
        Font::Osifont => font_file!("classics", "osifont-Regular.ttf"),
        Font::ArchitectsDaughter => font_file!("classics", "ArchitectsDaughter-Regular.ttf"),
        Font::Kalam => font_file!("classics", "Kalam-Regular.ttf"),
        Font::PatrickHand => font_file!("classics", "PatrickHand-Regular.ttf"),
    })
}

/// A shipped font's measures, read once from its file.
pub struct Metrics {
    /// Advance widths of the characters 0..=255 (Latin-1, as PDF's
    /// WinAnsi encoding takes them), in ems; a character the font lacks is
    /// `None`.
    pub widths: [Option<f64>; 256],
    /// The average advance, for characters past Latin-1.
    pub average: f64,
    /// Ascent, descent (negative), cap height, in ems.
    pub ascent: f64,
    pub descent: f64,
    pub cap: f64,
    /// Its box, in ems: x min, y min, x max, y max.
    pub bbox: [f64; 4],
    /// The PostScript name, letters and digits only.
    pub ps_name: String,
}

pub fn metrics(f: Font) -> Option<&'static Metrics> {
    static ALL: OnceLock<Vec<Option<Metrics>>> = OnceLock::new();
    let all = ALL.get_or_init(|| Font::ALL.iter().map(|f| read(*f)).collect());
    let i = Font::ALL.iter().position(|x| *x == f)?;
    all[i].as_ref()
}

fn read(f: Font) -> Option<Metrics> {
    let face = ttf_parser::Face::parse(bytes(f)?, 0).ok()?;
    let em = face.units_per_em() as f64;
    let mut widths = [None; 256];
    let (mut sum, mut n) = (0.0, 0.0);
    for (c, w) in widths.iter_mut().enumerate() {
        let Some(ch) = char::from_u32(c as u32) else {
            continue;
        };
        if let Some(g) = face.glyph_index(ch) {
            if let Some(a) = face.glyph_hor_advance(g) {
                let a = a as f64 / em;
                *w = Some(a);
                if ch.is_ascii_alphanumeric() {
                    sum += a;
                    n += 1.0;
                }
            }
        }
    }
    let b = face.global_bounding_box();
    let ps_name: String = f
        .family()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    Some(Metrics {
        widths,
        average: if n > 0.0 { sum / n } else { 0.55 },
        ascent: face.ascender() as f64 / em,
        descent: face.descender() as f64 / em,
        cap: face.capital_height().map_or(0.7, |h| h as f64 / em),
        bbox: [
            b.x_min as f64 / em,
            b.y_min as f64 / em,
            b.x_max as f64 / em,
            b.y_max as f64 / em,
        ],
        ps_name,
    })
}

/// How wide `text` runs in shipped font `f`, `size` tall (the size's
/// units); none for the PDF's own three.
pub fn width(f: Font, text: &str, size: f64) -> Option<f64> {
    let m = metrics(f)?;
    let ems: f64 = text
        .chars()
        .map(|c| {
            // a character the font lacks is drawn in Helvetica (PDF) or
            // the browser's own (SVG): measured as Helvetica
            let i = c as u32;
            let own = if i < 256 { m.widths[i as usize] } else { None };
            own.unwrap_or_else(|| crate::paper::text_width(&c.to_string(), 1.0))
        })
        .sum();
    Some(ems * size)
}

/// Bytes as base64, for a font inside an SVG.
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shipped_font_reads_and_measures() {
        for f in Font::ALL {
            if !f.bundled() {
                assert!(metrics(f).is_none());
                continue;
            }
            let m = metrics(f).unwrap_or_else(|| panic!("{f:?} reads"));
            assert!(m.widths[b'A' as usize].is_some(), "{f:?} has A");
            let w = width(f, "WALL", 2.5).unwrap();
            assert!(w > 2.0 && w < 12.0, "{f:?} {w}");
        }
    }

    #[test]
    fn base64_is_standard() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
    }
}

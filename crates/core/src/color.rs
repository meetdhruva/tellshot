//! Colours, serialised as `#rrggbb` or `#rrggbbaa` so files and agent payloads stay readable.

use alloc::string::String;
use core::fmt;

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A straight (non-premultiplied) sRGB colour with alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel; 255 is opaque.
    pub a: u8,
}

impl Color {
    /// An opaque colour.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// A colour with alpha.
    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// The same colour with a different alpha.
    #[must_use]
    pub const fn with_alpha(self, a: u8) -> Self {
        Self { a, ..self }
    }

    /// Parses `#rgb`, `#rrggbb` or `#rrggbbaa` (the `#` is optional).
    #[must_use]
    pub fn from_hex(s: &str) -> Option<Self> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        // from_str_radix accepts a leading '+', so check the digits explicitly.
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
        let nibble =
            |i: usize| hex.get(i..=i).and_then(|h| u8::from_str_radix(h, 16).ok()).map(|n| n * 17);
        match hex.len() {
            3 => Some(Self::rgb(nibble(0)?, nibble(1)?, nibble(2)?)),
            6 => Some(Self::rgb(byte(0)?, byte(2)?, byte(4)?)),
            8 => Some(Self::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }

    /// Formats as `#rrggbb`, or `#rrggbbaa` when not opaque.
    #[must_use]
    pub fn to_hex(self) -> String {
        if self.a == 255 {
            alloc::format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            alloc::format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
        }
    }

    /// WCAG 2.x relative luminance in `0.0..=1.0`.
    #[must_use]
    pub fn relative_luminance(self) -> f32 {
        fn channel(c: u8) -> f32 {
            let c = f32::from(c) / 255.0;
            if c <= 0.039_28 { c / 12.92 } else { libm::powf((c + 0.055) / 1.055, 2.4) }
        }
        let (r, g, b) = (channel(self.r), channel(self.g), channel(self.b));
        libm::fmaf(0.2126, r, libm::fmaf(0.7152, g, 0.0722 * b))
    }

    /// WCAG 2.x contrast ratio between two opaque colours, from 1.0 to 21.0.
    #[must_use]
    pub fn contrast_ratio(self, other: Self) -> f32 {
        let (a, b) = (self.relative_luminance(), other.relative_luminance());
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct HexVisitor;
        impl Visitor<'_> for HexVisitor {
            type Value = Color;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a colour like \"#d55e00\" or \"#d55e0080\"")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Color, E> {
                Color::from_hex(v).ok_or_else(|| E::invalid_value(de::Unexpected::Str(v), &self))
            }
        }
        deserializer.deserialize_str(HexVisitor)
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for Color {
    fn schema_name() -> alloc::borrow::Cow<'static, str> {
        "Color".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "sRGB colour as #rrggbb or #rrggbbaa",
            "pattern": "^#?([0-9a-fA-F]{3}|[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$",
            "examples": ["#d55e00", "#f0e44280"]
        })
    }
}

/// The default annotation palette.
///
/// Based on the Okabe–Ito palette, which stays distinguishable for the common forms of colour
/// vision deficiency. Annotations are read by many people besides their author, so the defaults
/// must work for everyone.
pub mod palette {
    use super::Color;

    /// Vermilion: the default for arrows and shapes.
    pub const VERMILION: Color = Color::rgb(0xd5, 0x5e, 0x00);
    /// Blue.
    pub const BLUE: Color = Color::rgb(0x00, 0x72, 0xb2);
    /// Sky blue.
    pub const SKY_BLUE: Color = Color::rgb(0x56, 0xb4, 0xe9);
    /// Bluish green.
    pub const BLUISH_GREEN: Color = Color::rgb(0x00, 0x9e, 0x73);
    /// Yellow: the default highlighter.
    pub const YELLOW: Color = Color::rgb(0xf0, 0xe4, 0x42);
    /// Orange.
    pub const ORANGE: Color = Color::rgb(0xe6, 0x9f, 0x00);
    /// Reddish purple.
    pub const REDDISH_PURPLE: Color = Color::rgb(0xcc, 0x79, 0xa7);
    /// Black: redaction is always opaque black.
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    /// White.
    pub const WHITE: Color = Color::rgb(0xff, 0xff, 0xff);

    /// Every swatch offered in the colour picker, in display order, with an accessible name.
    pub const SWATCHES: [(Color, &str); 9] = [
        (VERMILION, "Vermilion"),
        (BLUE, "Blue"),
        (SKY_BLUE, "Sky blue"),
        (BLUISH_GREEN, "Bluish green"),
        (YELLOW, "Yellow"),
        (ORANGE, "Orange"),
        (REDDISH_PURPLE, "Reddish purple"),
        (BLACK, "Black"),
        (WHITE, "White"),
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        for c in [palette::VERMILION, palette::YELLOW.with_alpha(0x80), Color::rgba(1, 2, 3, 4)] {
            assert_eq!(Color::from_hex(&c.to_hex()), Some(c), "{c}");
        }
        assert_eq!(Color::from_hex("#fff"), Some(palette::WHITE), "short form");
        assert_eq!(Color::from_hex("#ggg"), None, "invalid digits");
        assert_eq!(Color::from_hex("#1234"), None, "invalid length");
        assert_eq!(Color::from_hex("#éé"), None, "non-ascii must not panic");
    }

    #[test]
    fn serde_uses_hex_strings() {
        let json = serde_json::to_string(&palette::BLUE).unwrap();
        assert_eq!(json, "\"#0072b2\"", "serialised form");
        let back: Color = serde_json::from_str(&json).unwrap();
        assert_eq!(back, palette::BLUE, "round trip");
        assert!(serde_json::from_str::<Color>("\"red\"").is_err(), "named colours rejected");
    }

    #[test]
    fn contrast_matches_wcag_reference_values() {
        let ratio = palette::BLACK.contrast_ratio(palette::WHITE);
        assert!((ratio - 21.0).abs() < 0.01, "black on white is 21:1, got {ratio}");
        assert!((palette::BLUE.contrast_ratio(palette::BLUE) - 1.0).abs() < 1e-6, "self is 1:1");
    }
}

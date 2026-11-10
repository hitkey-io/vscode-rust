//! Colour arithmetic, ported from `vs/base/common/color.ts` so transformed
//! defaults (`darken`, `lighten`, `transparent`, `lessProminent`...) land on
//! exactly the values VS Code computes.

use egui::Color32;

fn round_float(n: f64, decimals: i32) -> f64 {
    let d = 10f64.powi(decimals);
    (n * d).round() / d
}

/// `RGBA`: channels 0–255 (truncated like `| 0`), alpha 0–1 rounded to 3
/// decimals.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

struct Hsla {
    h: f64,
    s: f64,
    l: f64,
    a: f64,
}

impl Hsla {
    fn new(h: f64, s: f64, l: f64, a: f64) -> Self {
        Hsla {
            h: h.clamp(0.0, 360.0).trunc(),
            s: round_float(s.clamp(0.0, 1.0), 3),
            l: round_float(l.clamp(0.0, 1.0), 3),
            a: round_float(a.clamp(0.0, 1.0), 3),
        }
    }

    fn from_rgba(c: &Color) -> Self {
        let r = c.r as f64 / 255.0;
        let g = c.g as f64 / 255.0;
        let b = c.b as f64 / 255.0;
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let mut h = 0.0;
        let mut s = 0.0;
        let l = (min + max) / 2.0;
        let chroma = max - min;
        if chroma > 0.0 {
            s = (if l <= 0.5 { chroma / (2.0 * l) } else { chroma / (2.0 - 2.0 * l) }).min(1.0);
            h = if max == r {
                (g - b) / chroma + if g < b { 6.0 } else { 0.0 }
            } else if max == g {
                (b - r) / chroma + 2.0
            } else {
                (r - g) / chroma + 4.0
            };
            h = (h * 60.0).round();
        }
        Hsla::new(h, s, l, c.a)
    }

    fn hue_to_rgb(p: f64, q: f64, mut t: f64) -> f64 {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            return p + (q - p) * 6.0 * t;
        }
        if t < 1.0 / 2.0 {
            return q;
        }
        if t < 2.0 / 3.0 {
            return p + (q - p) * (2.0 / 3.0 - t) * 6.0;
        }
        p
    }

    fn to_rgba(&self) -> Color {
        let h = self.h / 360.0;
        let (r, g, b) = if self.s == 0.0 {
            (self.l, self.l, self.l)
        } else {
            let q = if self.l < 0.5 { self.l * (1.0 + self.s) } else { self.l + self.s - self.l * self.s };
            let p = 2.0 * self.l - q;
            (
                Self::hue_to_rgb(p, q, h + 1.0 / 3.0),
                Self::hue_to_rgb(p, q, h),
                Self::hue_to_rgb(p, q, h - 1.0 / 3.0),
            )
        };
        Color::new((r * 255.0).round(), (g * 255.0).round(), (b * 255.0).round(), self.a)
    }
}

impl Color {
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 1.0 };
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 1.0 };
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0.0 };
    pub const RED: Color = Color { r: 255, g: 0, b: 0, a: 1.0 };

    pub fn new(r: f64, g: f64, b: f64, a: f64) -> Self {
        Color {
            r: r.clamp(0.0, 255.0) as u8,
            g: g.clamp(0.0, 255.0) as u8,
            b: b.clamp(0.0, 255.0) as u8,
            a: round_float(a.clamp(0.0, 1.0), 3),
        }
    }

    /// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` (`Color.Format.CSS.parseHex`).
    pub fn from_hex(hex: &str) -> Option<Color> {
        let h = hex.strip_prefix('#')?;
        let d = |i: usize| u8::from_str_radix(h.get(i..i + 1)?, 16).ok();
        let dd = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
        match h.len() {
            3 => Some(Color::new((d(0)? * 17) as f64, (d(1)? * 17) as f64, (d(2)? * 17) as f64, 1.0)),
            4 => Some(Color::new(
                (d(0)? * 17) as f64,
                (d(1)? * 17) as f64,
                (d(2)? * 17) as f64,
                (d(3)? * 17) as f64 / 255.0,
            )),
            6 => Some(Color::new(dd(0)? as f64, dd(2)? as f64, dd(4)? as f64, 1.0)),
            8 => Some(Color::new(dd(0)? as f64, dd(2)? as f64, dd(4)? as f64, dd(6)? as f64 / 255.0)),
            _ => None,
        }
    }

    /// What VS Code does with a value it can't parse: `Color.fromHex(x) || Color.red`.
    pub fn from_hex_or_red(hex: &str) -> Color {
        Color::from_hex(hex).unwrap_or(Color::RED)
    }

    pub fn to_hex(&self) -> String {
        if self.a >= 1.0 {
            format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
        } else {
            format!("#{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, (self.a * 255.0).round() as u8)
        }
    }

    pub fn to_color32(&self) -> Color32 {
        Color32::from_rgba_unmultiplied(self.r, self.g, self.b, (self.a * 255.0).round() as u8)
    }

    fn channel_luminance(c: u8) -> f64 {
        let c = c as f64 / 255.0;
        if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    }

    pub fn relative_luminance(&self) -> f64 {
        let l = 0.2126 * Self::channel_luminance(self.r)
            + 0.7152 * Self::channel_luminance(self.g)
            + 0.0722 * Self::channel_luminance(self.b);
        round_float(l, 4)
    }

    /// YIQ test (`Color.isDarker`): picks dark vs light text on a colour.
    pub fn is_darker(&self) -> bool {
        (self.r as f64 * 299.0 + self.g as f64 * 587.0 + self.b as f64 * 114.0) / 1000.0 < 128.0
    }

    pub fn is_darker_than(&self, other: &Color) -> bool {
        self.relative_luminance() < other.relative_luminance()
    }

    pub fn is_lighter_than(&self, other: &Color) -> bool {
        self.relative_luminance() > other.relative_luminance()
    }

    pub fn lighten(&self, factor: f64) -> Color {
        let h = Hsla::from_rgba(self);
        Hsla::new(h.h, h.s, h.l + h.l * factor, h.a).to_rgba()
    }

    pub fn darken(&self, factor: f64) -> Color {
        let h = Hsla::from_rgba(self);
        Hsla::new(h.h, h.s, h.l - h.l * factor, h.a).to_rgba()
    }

    pub fn transparent(&self, factor: f64) -> Color {
        Color::new(self.r as f64, self.g as f64, self.b as f64, self.a * factor)
    }

    pub fn with_opacity(&self, opacity: f64) -> Color {
        Color::new(self.r as f64, self.g as f64, self.b as f64, opacity)
    }

    pub fn is_opaque(&self) -> bool {
        self.a == 1.0
    }

    pub fn make_opaque(&self, bg: &Color) -> Color {
        if self.is_opaque() || bg.a != 1.0 {
            return *self;
        }
        let a = self.a;
        Color::new(
            bg.r as f64 - a * (bg.r as f64 - self.r as f64),
            bg.g as f64 - a * (bg.g as f64 - self.g as f64),
            bg.b as f64 - a * (bg.b as f64 - self.b as f64),
            1.0,
        )
    }

    pub fn mix(&self, other: &Color, factor: f64) -> Color {
        let f = factor.clamp(0.0, 1.0);
        let m = |x: f64, y: f64| x + (y - x) * f;
        Color::new(
            m(self.r as f64, other.r as f64),
            m(self.g as f64, other.g as f64),
            m(self.b as f64, other.b as f64),
            m(self.a, other.a),
        )
    }

    /// `Color.blend`: this colour composited over `c`.
    pub fn blend(&self, c: &Color) -> Color {
        let ta = self.a;
        let ca = c.a;
        let a = ta + ca * (1.0 - ta);
        if a < 1e-6 {
            return Color::TRANSPARENT;
        }
        let ch = |x: u8, y: u8| x as f64 * ta / a + y as f64 * ca * (1.0 - ta) / a;
        Color::new(ch(self.r, c.r), ch(self.g, c.g), ch(self.b, c.b), a)
    }

    pub fn lighter_color(of: &Color, relative: &Color, factor: f64) -> Color {
        if of.is_lighter_than(relative) {
            return *of;
        }
        let factor = if factor == 0.0 { 0.5 } else { factor };
        let l1 = of.relative_luminance();
        let l2 = relative.relative_luminance();
        of.lighten(factor * (l2 - l1) / l2)
    }

    pub fn darker_color(of: &Color, relative: &Color, factor: f64) -> Color {
        if of.is_darker_than(relative) {
            return *of;
        }
        let factor = if factor == 0.0 { 0.5 } else { factor };
        let l1 = of.relative_luminance();
        let l2 = relative.relative_luminance();
        of.darken(factor * (l1 - l2) / l1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        assert_eq!(Color::from_hex("#3994BCB3").unwrap().to_hex(), "#3994bcb3");
        assert_eq!(Color::from_hex("#fff").unwrap(), Color::WHITE);
        assert_eq!(Color::from_hex("nope"), None);
    }

    #[test]
    fn transforms_match_vs_code() {
        // Expected values computed with VS Code's own color.ts.
        let c = Color::from_hex("#252526").unwrap();
        assert_eq!(c.lighten(0.2).to_hex(), "#2c2c2d");
        assert_eq!(Color::from_hex("#3994BC").unwrap().darken(0.3).to_hex(), "#286884");
        assert_eq!(Color::from_hex("#E5BA7D").unwrap().lighten(0.45).to_hex(), "#ffffff");
        assert_eq!(Color::BLACK.lighten(0.2), Color::BLACK);
        assert_eq!(Color::WHITE.transparent(0.12).to_hex(), "#ffffff1f");
        let wash = Color::from_hex("#ffffff1f").unwrap().make_opaque(&Color::from_hex("#181818").unwrap());
        assert_eq!(wash.to_hex(), "#343434");
    }
}

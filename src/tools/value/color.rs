//! 颜色互转：HEX / RGB / RGBA / HSL / Java / Android。

use crate::tools::meta::{Group, ToolMeta};
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct Color;

#[derive(Clone, Copy)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: f64,
}

const NAMED: [(&str, u32); 16] = [
    ("black", 0x000000),
    ("white", 0xFFFFFF),
    ("red", 0xFF0000),
    ("lime", 0x00FF00),
    ("blue", 0x0000FF),
    ("yellow", 0xFFFF00),
    ("cyan", 0x00FFFF),
    ("magenta", 0xFF00FF),
    ("silver", 0xC0C0C0),
    ("gray", 0x808080),
    ("maroon", 0x800000),
    ("olive", 0x808000),
    ("green", 0x008000),
    ("purple", 0x800080),
    ("teal", 0x008080),
    ("navy", 0x000080),
];

fn parse_hex(s: &str) -> Option<Rgba> {
    let h = s.trim_start_matches('#');
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full = match h.len() {
        3 | 4 => h.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 | 8 => h.to_string(),
        _ => return None,
    };
    let parse = |i: usize| u8::from_str_radix(&full[i..i + 2], 16).ok();
    if full.len() == 6 {
        Some(Rgba {
            r: parse(0)?,
            g: parse(2)?,
            b: parse(4)?,
            a: 1.0,
        })
    } else {
        Some(Rgba {
            r: parse(2)?,
            g: parse(4)?,
            b: parse(6)?,
            a: parse(0)? as f64 / 255.0,
        })
    }
}

fn numbers(s: &str) -> Vec<f64> {
    let inner = s
        .split_once('(')
        .map(|(_, rest)| rest.trim_end_matches(')'))
        .unwrap_or("");
    inner
        .split([',', '/', ' '])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter_map(|p| {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f64>().ok().map(|v| v / 100.0)
            } else {
                p.parse::<f64>().ok()
            }
        })
        .collect()
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0) / 360.0;
    let (r, g, b) = if s == 0.0 {
        (l, l, l)
    } else {
        let q = if l < 0.5 {
            l * (1.0 + s)
        } else {
            l + s - l * s
        };
        let p = 2.0 * l - q;
        let hue = |mut t: f64| {
            if t < 0.0 {
                t += 1.0;
            }
            if t > 1.0 {
                t -= 1.0;
            }
            if t < 1.0 / 6.0 {
                p + (q - p) * 6.0 * t
            } else if t < 0.5 {
                q
            } else if t < 2.0 / 3.0 {
                p + (q - p) * (2.0 / 3.0 - t) * 6.0
            } else {
                p
            }
        };
        (hue(h + 1.0 / 3.0), hue(h), hue(h - 1.0 / 3.0))
    };
    (
        (r * 255.0).round().clamp(0.0, 255.0) as u8,
        (g * 255.0).round().clamp(0.0, 255.0) as u8,
        (b * 255.0).round().clamp(0.0, 255.0) as u8,
    )
}

fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if max == r {
        ((g - b) / d + if g < b { 6.0 } else { 0.0 }) / 6.0
    } else if max == g {
        ((b - r) / d + 2.0) / 6.0
    } else {
        ((r - g) / d + 4.0) / 6.0
    };
    (h * 360.0, s, l)
}

fn parse_color(s: &str) -> Option<Rgba> {
    let t = s.trim();
    let lower = t.to_ascii_lowercase();
    if let Some((_, hex)) = NAMED.iter().find(|(name, _)| *name == lower) {
        return parse_hex(&format!("{hex:06X}"));
    }
    if lower.starts_with("rgb") || lower.starts_with("hsl") {
        let nums = numbers(&lower);
        if lower.starts_with("hsl") {
            if nums.len() < 3 {
                return None;
            }
            let (r, g, b) = hsl_to_rgb(nums[0], nums[1], nums[2]);
            return Some(Rgba {
                r,
                g,
                b,
                a: nums.get(3).copied().unwrap_or(1.0),
            });
        }
        if nums.len() < 3 {
            return None;
        }
        return Some(Rgba {
            r: nums[0].clamp(0.0, 255.0) as u8,
            g: nums[1].clamp(0.0, 255.0) as u8,
            b: nums[2].clamp(0.0, 255.0) as u8,
            a: nums.get(3).copied().unwrap_or(1.0),
        });
    }
    parse_hex(t)
}

fn luminance(c: &Rgba) -> f64 {
    let f = |v: u8| {
        let s = v as f64 / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * f(c.r) + 0.7152 * f(c.g) + 0.0722 * f(c.b)
}

fn format_color(c: &Rgba) -> String {
    let (h, s, l) = rgb_to_hsl(c.r, c.g, c.b);
    let alpha = (c.a * 255.0).round().clamp(0.0, 255.0) as u8;
    let name = NAMED
        .iter()
        .find(|(_, v)| {
            let (r, g, b) = (
                (v >> 16 & 0xFF) as u8,
                (v >> 8 & 0xFF) as u8,
                (v & 0xFF) as u8,
            );
            r == c.r && g == c.g && b == c.b
        })
        .map(|(n, _)| *n);
    let lum = luminance(c);
    format!(
        "HEX        #{:02X}{:02X}{:02X}\nRGB        rgb({}, {}, {})\nRGBA       rgba({}, {}, {}, {:.2})\nHSL        hsl({:.1}, {:.1}%, {:.1}%)\nJava       new Color({}, {}, {})\nAndroid    0x{:02X}{:02X}{:02X}{:02X}\n相对亮度   {:.3}（{}）{}",
        c.r,
        c.g,
        c.b,
        c.r,
        c.g,
        c.b,
        c.r,
        c.g,
        c.b,
        c.a,
        h,
        s * 100.0,
        l * 100.0,
        c.r,
        c.g,
        c.b,
        alpha,
        c.r,
        c.g,
        c.b,
        lum,
        if lum > 0.179 {
            "建议深色文字"
        } else {
            "建议浅色文字"
        },
        match name {
            Some(n) => format!("\n颜色名     {n}"),
            None => String::new(),
        }
    )
}

impl Tool for Color {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "color",
            "颜色互转",
            Group::Value,
            "HEX / RGB / HSL / Java / Android 互转，并给出亮度与文字颜色建议",
            "#FF8800\nrgb(255, 136, 0)\nhsl(32, 100%, 50%)",
            true,
            "text",
        )
    }

    fn run(&self, input: &str, _ctx: &RunCtx) -> Result<ToolResult, String> {
        let mut out = Vec::new();
        for line in input
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .take(50)
        {
            match parse_color(line) {
                Some(c) => out.push(format!("{line}\n{}", format_color(&c))),
                None => out.push(format!(
                    "{line}\n  看不懂，支持 #RGB / #RRGGBB / #AARRGGBB / rgb() / hsl() / 颜色名"
                )),
            }
        }
        if out.is_empty() {
            return Err("输入一个颜色，例如 #FF8800".to_string());
        }
        Ok(ToolResult::new()
            .section("转换结果", "text", out.join("\n\n"))
            .note("8 位 HEX 按 Android 的 AARRGGBB 解析"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn ctx() -> RunCtx<'static> {
        let leaked: &'static Value = Box::leak(Box::new(json!({})));
        RunCtx {
            options: leaked,
            now_ms: 0,
        }
    }

    #[test]
    fn hex_to_rgb() {
        let c = parse_color("#FF8800").unwrap();
        assert_eq!((c.r, c.g, c.b), (255, 136, 0));
        let text = format_color(&c);
        assert!(text.contains("rgb(255, 136, 0)"));
        assert!(text.contains("0xFFFF8800"));
    }

    #[test]
    fn rgb_and_hsl_parse() {
        let c = parse_color("rgb(255, 136, 0)").unwrap();
        assert_eq!((c.r, c.g, c.b), (255, 136, 0));
        let c = parse_color("hsl(32, 100%, 50%)").unwrap();
        assert_eq!((c.r, c.g, c.b), (255, 136, 0));
    }

    #[test]
    fn named_and_short_hex() {
        assert_eq!(parse_color("red").unwrap().r, 255);
        assert_eq!(parse_color("#f80").unwrap().g, 136);
    }

    #[test]
    fn tool_runs() {
        let r = Color.run("#000000", &ctx()).unwrap();
        assert!(r.sections[0].text.contains("建议浅色文字"));
    }
}

//! Finite sRGB output for common CSS Color 4 theme inputs. Display-P3 matrices
//! follow CSSWG css-color-4/conversions.js; wide gamut is clipped for Slint.
fn number(v: &str) -> Option<f64> {
    v.parse::<f64>().ok().filter(|v| v.is_finite())
}
fn ratio(v: &str) -> Option<f64> {
    if let Some(v) = v.strip_suffix('%') {
        Some(number(v)? / 100.)
    } else {
        number(v)
    }
}
fn angle(v: &str) -> Option<f64> {
    if let Some(v) = v.strip_suffix("turn") {
        Some(number(v)? * 360.)
    } else if let Some(v) = v.strip_suffix("rad") {
        Some(number(v)?.to_degrees())
    } else {
        number(v.strip_suffix("deg").unwrap_or(v))
    }
}
fn parts(body: &str) -> Vec<&str> {
    body.split(|c: char| c == ',' || c == '/' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect()
}
fn pack(rgb: [f64; 3], alpha: f64) -> Option<u32> {
    if !rgb.iter().all(|v| v.is_finite()) || !alpha.is_finite() {
        return None;
    }
    let b = |x: f64| (x.clamp(0., 1.) * 255.).round() as u32;
    Some(b(alpha) << 24 | b(rgb[0]) << 16 | b(rgb[1]) << 8 | b(rgb[2]))
}
fn decode(v: f64) -> f64 {
    if v.abs() <= 0.04045 {
        v / 12.92
    } else {
        v.signum() * ((v.abs() + 0.055) / 1.055).powf(2.4)
    }
}
fn encode(v: f64) -> f64 {
    if v.abs() <= 0.0031308 {
        v * 12.92
    } else {
        v.signum() * (1.055 * v.abs().powf(1. / 2.4) - 0.055)
    }
}
fn matrix(m: [[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    m.map(|r| r[0] * v[0] + r[1] * v[1] + r[2] * v[2])
}
pub fn parse(value: &str) -> Option<u32> {
    let named = match value {
        "red" => Some(0xff0000),
        "green" => Some(0x008000),
        "blue" => Some(0x0000ff),
        "yellow" => Some(0xffff00),
        "cyan" | "aqua" => Some(0x00ffff),
        "magenta" | "fuchsia" => Some(0xff00ff),
        "gray" | "grey" => Some(0x808080),
        "orange" => Some(0xffa500),
        "purple" => Some(0x800080),
        "pink" => Some(0xffc0cb),
        "lime" => Some(0x00ff00),
        "navy" => Some(0x000080),
        "teal" => Some(0x008080),
        "silver" => Some(0xc0c0c0),
        "maroon" => Some(0x800000),
        "olive" => Some(0x808000),
        "rebeccapurple" => Some(0x663399),
        _ => None,
    };
    if let Some(color) = named {
        return Some(color | 0xff000000);
    }
    let (name, body) = value.split_once('(')?;
    let body = body.strip_suffix(')')?;
    let p = parts(body);
    if name == "hsl" || name == "hsla" {
        if !(3..=4).contains(&p.len()) {
            return None;
        }
        let h = angle(p[0])?.rem_euclid(360.) / 60.;
        let s = number(p[1].strip_suffix('%')?)?.clamp(0., 100.) / 100.;
        let l = number(p[2].strip_suffix('%')?)?.clamp(0., 100.) / 100.;
        let c = (1. - (2. * l - 1.).abs()) * s;
        let x = c * (1. - (h % 2. - 1.).abs());
        let m = l - c / 2.;
        let rgb = match h as u32 {
            0 => [c, x, 0.],
            1 => [x, c, 0.],
            2 => [0., c, x],
            3 => [0., x, c],
            4 => [x, 0., c],
            _ => [c, 0., x],
        };
        return pack(rgb.map(|v| v + m), p.get(3).map_or(Some(1.), |v| ratio(v))?);
    }
    if name == "color" && p.first() == Some(&"display-p3") {
        if !(4..=5).contains(&p.len()) {
            return None;
        }
        let rgb = [ratio(p[1])?, ratio(p[2])?, ratio(p[3])?].map(decode);
        let xyz = matrix(
            [
                [608311. / 1250200., 189793. / 714400., 198249. / 1000160.],
                [35783. / 156275., 247089. / 357200., 198249. / 2500400.],
                [0., 32229. / 714400., 5220557. / 5000800.],
            ],
            rgb,
        );
        let rgb = matrix(
            [
                [12831. / 3959., -329. / 214., -1974. / 3959.],
                [-851781. / 878810., 1648619. / 878810., 36519. / 878810.],
                [705. / 12673., -2585. / 12673., 705. / 667.],
            ],
            xyz,
        )
        .map(encode);
        return pack(rgb, p.get(4).map_or(Some(1.), |v| ratio(v))?);
    }
    if name == "oklch" {
        if !(3..=4).contains(&p.len()) {
            return None;
        }
        let l = ratio(p[0])?.clamp(0., 1.);
        let c = if let Some(v) = p[1].strip_suffix('%') {
            number(v)? * 0.004
        } else {
            number(p[1])?
        }
        .max(0.);
        let h = angle(p[2])?.to_radians();
        let (a, b) = (c * h.cos(), c * h.sin());
        let lms = [
            l + 0.3963377774 * a + 0.2158037573 * b,
            l - 0.1055613458 * a - 0.0638541728 * b,
            l - 0.0894841775 * a - 1.291485548 * b,
        ]
        .map(|v| v * v * v);
        let rgb = matrix(
            [
                [4.0767416621, -3.3077115913, 0.2309699292],
                [-1.2684380046, 2.6097574011, -0.3413193965],
                [-0.0041960863, -0.7034186147, 1.707614701],
            ],
            lms,
        )
        .map(encode);
        return pack(rgb, p.get(3).map_or(Some(1.), |v| ratio(v))?);
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn css_color_spaces_have_known_srgb_results_and_reject_nonfinite_values() {
        assert_eq!(parse("hsl(120 100% 50% / 25%)"), Some(0x4000ff00));
        assert_eq!(parse("hsla(0.5turn,100%,50%,1)"), Some(0xff00ffff));
        assert_eq!(parse("color(display-p3 1 1 1 / .2)"), Some(0x33ffffff));
        assert_eq!(parse("color(display-p3 1 0 0)"), Some(0xffff0000));
        assert_eq!(parse("oklch(100% 0 0)"), Some(0xffffffff));
        assert_eq!(parse("oklch(0 0 90)"), Some(0xff000000));
        assert!(parse("color(display-p3 NaN 0 0)").is_none());
        assert!(parse("oklch(.5 inf 0)").is_none());
    }
}

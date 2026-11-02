//! Draft-only appearance editing. Preview work never changes the song or store.
use crate::{themes, BraccatoRow, BraccatoWord, MainWindow};
#[cfg(not(target_arch = "wasm32"))]
use slint::Model;
use slint::{ComponentHandle, ModelRc, VecModel};
use std::rc::Rc;

const BEGIN: &str = "/* vey-appearance-controls:start */";
const END: &str = "/* vey-appearance-controls:end */";

fn managed(source: &str, name: &str, value: &str) -> String {
    let (base, mut values) = if let Some((base, rest)) = source.split_once(BEGIN) {
        if let Some((block, after)) = rest.split_once(END) {
            let values = block
                .trim()
                .trim_start_matches(":root")
                .trim()
                .trim_start_matches('{')
                .trim_end_matches('}')
                .split(';')
                .filter_map(|d| d.split_once(':'))
                .map(|(n, v)| (n.trim().to_string(), v.trim().to_string()))
                .collect::<std::collections::BTreeMap<_, _>>();
            (format!("{base}{after}"), values)
        } else {
            (source.to_string(), Default::default())
        }
    } else {
        (source.to_string(), Default::default())
    };
    values.insert(name.into(), value.into());
    format!(
        "{}\n{BEGIN}\n:root {{\n{}\n}}\n{END}",
        base.trim_end(),
        values
            .iter()
            .map(|(n, v)| format!("  {n}: {v};"))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

pub fn draft(w: &MainWindow) -> Result<themes::NativeTheme, String> {
    let source = w.get_lyric_theme_source();
    if source.len() > 65536 {
        return Err("Theme exceeds 64 KiB".into());
    }
    let mut theme = themes::convert(&source);
    // The browser owns its full upstream RICS compiler. Native-only language
    // limitations must not prevent saving a theme that it can render there.
    #[cfg(target_arch = "wasm32")]
    if theme
        .errors
        .iter()
        .all(|e| e.starts_with("Unsupported RICS directive:"))
    {
        theme.unsupported.append(&mut theme.errors);
    }
    if !theme.errors.is_empty() {
        return Err(theme.errors.join("\n"));
    }
    let font = w.get_lyric_font_family();
    if !font.trim().is_empty() {
        theme.appearance.family = font.trim().into();
    }
    Ok(theme)
}

fn model<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
    Rc::new(VecModel::from(values)).into()
}
fn sample_family(text: &str, preferred: &str) -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::native_lyric_fonts::family_for(text, preferred)
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = text;
        preferred.into()
    }
}

pub fn preview(w: &MainWindow) {
    w.set_lyric_preset_index(crate::theme_presets::index_for(&w.get_lyric_theme_source()) as i32);
    match draft(w) {
        Err(error) => {
            w.set_lyric_draft_valid(false);
            w.set_lyric_style_status(error.into());
        }
        Ok(t) => {
            w.set_player_theme_draft(crate::player_themes::resolve(
                &t,
                w.get_theme_entire_player_draft(),
            ));
            w.set_lyric_draft_valid(true);
            w.set_lyric_style_status(if t.unsupported.is_empty() {
                "".into()
            } else {
                format!(
                    "{}: {}",
                    crate::i18n::translate(
                        &w.global::<crate::I18n>().get_language(),
                        "Не підтримується"
                    ),
                    t.unsupported.join("; ")
                )
                .into()
            });
            w.set_lyric_draft_style(t.style());
            w.set_lyric_draft_size(t.size);
            w.set_lyric_draft_weight(t.weight);
            w.set_lyric_draft_active(slint::Color::from_argb_encoded(t.active));
            w.set_lyric_draft_inactive(slint::Color::from_argb_encoded(t.inactive));
            let parts = ["Feel\u{a0}", "music"]
                .iter()
                .enumerate()
                .map(|(i, text)| BraccatoWord {
                    text: (*text).into(),
                    start_ms: i as f32 * 1800.,
                    duration_ms: 1800.,
                    cue_span: 1.,
                    active: true,
                    motion_start_ms: i as f32 * 1800.,
                    motion_duration_ms: 1800.,
                    font_family: sample_family(text, &t.appearance.family).into(),
                    ..Default::default()
                })
                .collect();
            let next = crate::i18n::translate(
                &w.global::<crate::I18n>().get_language(),
                "Наступний рядок",
            );
            let next_family = sample_family(&next, &t.appearance.family);
            w.set_lyric_preview_rows(model(vec![
                BraccatoRow {
                    text: "Feel music".into(),
                    rendered: true,
                    active: true,
                    start_ms: 0.,
                    end_ms: 4200.,
                    parts: model(parts),
                    font_family: sample_family("Feel music", &t.appearance.family).into(),
                    source_index: 0,
                    ..Default::default()
                },
                BraccatoRow {
                    text: next.into(),
                    rendered: true,
                    start_ms: 5000.,
                    end_ms: 8000.,
                    font_family: next_family.into(),
                    source_index: 1,
                    ..Default::default()
                },
            ]));
        }
    }
}

/// Install callbacks once, for either native or WASM window controllers.
pub fn install(w: &MainWindow) {
    let weak = w.as_weak();
    w.on_preview_lyric_style(move || {
        if let Some(w) = weak.upgrade() {
            preview(&w);
        }
    });
    let weak = w.as_weak();
    w.on_adjust_lyric_style(move |key, value| {
        if let Some(w) = weak.upgrade() {
            if !value.is_finite() {
                return;
            }
            let (name, value) = match key.as_str() {
                "corner" => (
                    "--vey-corner-radius",
                    format!("{}px", value.round().clamp(0., 40.)),
                ),
                "artwork-corner" => (
                    "--vey-artwork-radius",
                    format!("{}px", value.round().clamp(0., 40.)),
                ),
                "artwork-share" => (
                    "--vey-artwork-share",
                    format!("{:.2}", value.clamp(0.32, 0.5)),
                ),
                "ambient" => (
                    "--vey-ambient-strength",
                    format!("{:.2}", value.clamp(0., 1.5)),
                ),
                "size" => (
                    "--blyrics-font-size",
                    format!("{}px", value.round().clamp(12., 72.)),
                ),
                "weight" => (
                    "--blyrics-font-weight",
                    format!("{}", (value / 100.).round().clamp(1., 9.) * 100.),
                ),
                "spacing" => (
                    "--blyrics-line-height",
                    format!("{:.2}", value.clamp(1., 2.)),
                ),
                "blur" => (
                    "--blyrics-inactive-blur",
                    format!("{:.1}px", value.clamp(0., 12.)),
                ),
                "radius" => (
                    "--blyrics-highlight-glow-radius-from",
                    format!("{:.1}px", value.clamp(0., 32.)),
                ),
                "glow" => (
                    "--blyrics-animate-highlight-glow",
                    format!("{}", i32::from(value > 0.)),
                ),
                "wave" => (
                    "--blyrics-animate-letter-wave",
                    format!("{}", i32::from(value > 0.)),
                ),
                "wobble" => (
                    "--blyrics-animate-word-wobble",
                    format!("{}", i32::from(value > 0.)),
                ),
                "fade" => (
                    "--blyrics-animate-highlight-fade",
                    format!("{}", i32::from(value > 0.)),
                ),
                _ => return,
            };
            if !value.is_empty() {
                w.set_lyric_theme_source(managed(&w.get_lyric_theme_source(), name, &value).into());
            }
        }
    });
    let weak = w.as_weak();
    w.on_adjust_player_color(move |key, value| {
        if let Some(w) = weak.upgrade() {
            let name = match key.as_str() {
                "accent" => "--vey-accent",
                "background" => "--vey-background",
                "surface" => "--vey-surface",
                _ => return,
            };
            w.set_lyric_theme_source(
                managed(
                    &w.get_lyric_theme_source(),
                    name,
                    &format!("#{:06x}", value.as_argb_encoded() & 0xffffff),
                )
                .into(),
            );
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    install_paint(w);
}

#[cfg(not(target_arch = "wasm32"))]
fn install_paint(w: &MainWindow) {
    let weak = w.as_weak();
    let mut paint = crate::native_lyric_paint::WordPaint::default();
    let mut focus = crate::native_lyric_focus::FocusPaint::default();
    let mut previous = ModelRc::<BraccatoRow>::default();
    let mut prepared = [false; 2];
    w.on_lyric_preview_paint(move || {
        let Some(w) = weak.upgrade() else {
            return;
        };
        if w.get_dialog() != "lyric-style" {
            return;
        }
        let rows = w.get_lyric_preview_rows();
        if rows != previous {
            paint.reset();
            focus.reset();
            prepared.fill(false);
            previous = rows.clone();
            focus.sync(&rows, 0, 0, true);
        }
        let style = w.get_lyric_draft_style();
        if let Some((_, part)) = paint.poll(&rows, 1800, true) {
            prepared[part] = true;
        }
        if !paint.busy() {
            if let Some(part) = prepared.iter().position(|p| !p) {
                paint.submit_style(
                    &rows,
                    0,
                    part,
                    w.get_lyric_draft_size() * 0.62,
                    w.get_lyric_draft_weight() as u16,
                    &style,
                );
            }
        }
        focus.poll(&rows);
        focus.prepare_one(
            &rows,
            w.get_lyric_draft_size() * 0.62,
            w.get_lyric_draft_weight() as u16,
            style.inactive_blur,
        );
        if let Some(mut next) = rows.row_data(1) {
            if next.sustain_blurred.size().width > 0 && next.focus_mix != 1. {
                next.focus_mix = 1.;
                rows.set_row_data(1, next);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visual_overrides_preserve_rics_and_timing_and_do_not_accumulate() {
        let original =
            "$ink:#abc; :root { --blyrics-active-color:$ink; --blyrics-timing-offset:150ms; }";
        let mut source = original.to_string();
        for i in 0..100 {
            source = managed(
                &source,
                "--blyrics-font-size",
                &format!("{}px", 30 + i % 10),
            );
        }
        source = managed(&source, "--blyrics-animate-letter-wave", "1");
        assert!(source.starts_with(original));
        assert_eq!(source.matches(BEGIN).count(), 1);
        let theme = themes::convert(&source);
        assert!(
            theme.errors.is_empty() && theme.unsupported.is_empty(),
            "{theme:?}"
        );
        assert_eq!(theme.size, 39.);
        assert_eq!(theme.active, 0xffaabbcc);
        assert_eq!(theme.settings.line_offset_ms, 150);
        assert!(theme.appearance.wave);
    }
}

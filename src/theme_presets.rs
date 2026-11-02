//! Offline lyric presets shared by native and browser controllers.
use crate::{themes::NativeTheme, LyricThemePreset, MainWindow};
use slint::{ModelRc, VecModel};
use std::{rc::Rc, sync::OnceLock};

pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub creator: &'static str,
    pub reference: &'static str,
    pub css: &'static str,
}
pub fn all() -> &'static [Preset] {
    PRESETS
}
pub fn index_for(source: &str) -> usize {
    PRESETS
        .iter()
        .position(|p| p.css.trim() == source.trim())
        .unwrap_or(PRESETS.len())
}
static PRESETS: &[Preset] = &[
    Preset { id: "braccato", name: "Braccato", description: "Збалансоване сяйво й плавний рух слів.", creator: "Vey", reference: "", css: "" },
    Preset { id: "soft-glow", name: "М’яке сяйво", description: "М’яке біле сяйво та повільне згасання.", creator: "Vey", reference: "", css: ":root { --blyrics-glow-color:#ffffff40; --blyrics-highlight-glow-radius-from:8px; --blyrics-highlight-glow-min-duration:1.5s; }" },
    Preset { id: "crisp", name: "Чіткий текст", description: "Контрастний текст без декоративного руху.", creator: "Vey", reference: "", css: ":root { --blyrics-animate-highlight-glow:0; --blyrics-animate-word-wobble:0; --blyrics-animate-letter-wave:0; }" },
    Preset { id: "aurora", name: "Aurora", description: "М’ятне сяйво та м’який рух.", creator: "Vey", reference: "", css: include_str!("../assets/lyric-themes/aurora.css") },
    Preset { id: "ember", name: "Ember", description: "Теплий бурштин і делікатне підсвічування.", creator: "Vey", reference: "", css: include_str!("../assets/lyric-themes/ember.css") },
    Preset { id: "ocean", name: "Ocean", description: "Блакитний текст, легкі хвилі та простір.", creator: "Vey", reference: "", css: include_str!("../assets/lyric-themes/ocean.css") },
    Preset { id: "rose-quartz", name: "Rose Quartz", description: "Ніжні рожеві відтінки та плавне сяйво.", creator: "Vey", reference: "", css: include_str!("../assets/lyric-themes/rose-quartz.css") },
    Preset { id: "neon-pulse", name: "Neon Pulse", description: "Неоновий акцент і виразні хвилі літер.", creator: "Vey", reference: "", css: include_str!("../assets/lyric-themes/neon-pulse.css") },
    Preset { id: "nocturne", name: "Nocturne", description: "Спокійний нічний текст без зайвого руху.", creator: "Vey", reference: "", css: include_str!("../assets/lyric-themes/nocturne.css") },
    Preset { id: "minimal", name: "Minimal", description: "Чіткі слова без сяйва та коливань.", creator: "semicolonhope", reference: "https://github.com/ramansg/Minimal/tree/2c47d0512b39bc806a05716e5abdc8a7d73fb7f5", css: include_str!("../assets/lyric-themes/minimal.css") },
    Preset { id: "opacity-based-karaoke", name: "Opacity Karaoke", description: "Точна заливка з мінімумом ефектів.", creator: "semicolonhope", reference: "https://github.com/ramansg/Max-Performance-BlyricsTheme/tree/f5b7148ef45150e8afed4c6ccb298c52ced5701d", css: include_str!("../assets/lyric-themes/opacity-based-karaoke.css") },
    Preset { id: "apple-music", name: "Apple Music", description: "Великі білі рядки та стриманий фокус.", creator: "tposejank", reference: "https://github.com/tposejank/blyrics-am-theme/tree/64191f7f096cc5e05e19b36de5c69780ff536c30", css: include_str!("../assets/lyric-themes/apple-music.css") },
    Preset { id: "lucid", name: "Lucid", description: "Легкий фокус і прозорі неактивні рядки.", creator: "drago-oo", reference: "https://github.com/drago-oo/lucid-theme/tree/193735d5f5de36ea5110417b243e46967f754964", css: include_str!("../assets/lyric-themes/lucid.css") },
    Preset { id: "spotlight", name: "Spotlight", description: "Яскравий поточний рядок і розмитий фон.", creator: "boidushya", reference: "https://github.com/better-lyrics/theme-spotlight/tree/5ba22d84039aba95d00426114fea8077a63eaf03", css: include_str!("../assets/lyric-themes/spotlight.css") },
    Preset { id: "sustain", name: "Sustain", description: "Легкий шрифт і короткі спокійні хвилі.", creator: "boidushya", reference: "https://github.com/better-lyrics/theme-sustain/tree/503f7cbf1f8c40af5db20970d07cdca233dda4eb", css: include_str!("../assets/lyric-themes/sustain.css") },
    Preset { id: "material3", name: "Material 3", description: "М’який бузковий акцент і округлий фокус.", creator: "adidotzip", reference: "https://github.com/adidotzip/Material3/tree/27624e01549beaf2f7747d3e3a44b1126e8eab6d", css: include_str!("../assets/lyric-themes/material3.css") },
    Preset { id:"graphite",name:"Graphite",description:"Графітовий інтерфейс і чистий текст.",creator:"Vey",reference:"",css:include_str!("../assets/lyric-themes/graphite.css") },
    Preset { id:"solar-dusk",name:"Solar Dusk",description:"Тепле мідне світло й м’які форми.",creator:"Vey",reference:"",css:include_str!("../assets/lyric-themes/solar-dusk.css") },
    Preset { id:"cobalt",name:"Cobalt",description:"Глибокий синій фон і чіткі акценти.",creator:"Vey",reference:"",css:include_str!("../assets/lyric-themes/cobalt.css") },
    Preset { id:"paper-night",name:"Paper Night",description:"Теплий паперовий текст у нічній палітрі.",creator:"Vey",reference:"",css:include_str!("../assets/lyric-themes/paper-night.css") },
];

fn row(preset: Option<&Preset>, theme: &NativeTheme, language: &str) -> LyricThemePreset {
    let tr = |s: &str| crate::i18n::translate(language, s).into();
    LyricThemePreset {
        name: tr(preset.map_or("Власна тема", |p| p.name)),
        description: tr(preset.map_or(
            "Збережіть власні кольори, шрифти та ефекти.",
            |p| p.description,
        )),
        attribution: preset.map_or_else(
            || tr("Власна тема"),
            |p| {
                if p.reference.is_empty() {
                    tr("Оригінальна тема Vey")
                } else {
                    format!(
                        "{} · {}",
                        crate::i18n::translate(language, "Адаптація Better Lyrics"),
                        p.creator
                    )
                    .into()
                }
            },
        ),
        css: theme.source.clone().into(),
        active: slint::Color::from_argb_encoded(theme.active),
        inactive: slint::Color::from_argb_encoded(theme.inactive),
        glow: theme.style().glow_color,
        family: theme.appearance.family.clone().into(),
        weight: theme.weight,
    }
}
pub fn install(w: &MainWindow, language: &str, source: &str, current: &NativeTheme) {
    static COMPILED: OnceLock<Vec<NativeTheme>> = OnceLock::new();
    let compiled = COMPILED.get_or_init(|| {
        PRESETS
            .iter()
            .map(|p| crate::themes::convert(p.css))
            .collect()
    });
    let mut rows: Vec<_> = PRESETS
        .iter()
        .zip(compiled)
        .map(|(p, t)| row(Some(p), t, language))
        .collect();
    rows.push(row(None, current, language));
    let names = rows.iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    w.set_lyric_theme_names(ModelRc::new(Rc::new(VecModel::from(names))));
    w.set_lyric_theme_presets(ModelRc::new(Rc::new(VecModel::from(rows))));
    w.set_lyric_preset_index(index_for(source) as i32);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::Settings;
    #[test]
    fn presets_compile_without_unsupported_rules_or_timing_changes() {
        let mut ids = std::collections::HashSet::new();
        let baseline = NativeTheme::default().settings;
        for preset in all() {
            assert!(ids.insert(preset.id), "duplicate preset {}", preset.id);
            let theme = crate::themes::convert(preset.css);
            assert!(
                theme.unsupported.is_empty(),
                "{}: {:?}",
                preset.id,
                theme.unsupported
            );
            assert_eq!(
                serde_json::to_value(&theme.settings).unwrap(),
                serde_json::to_value(&baseline).unwrap(),
                "{} must not change timing or follow policy",
                preset.id
            );
            assert!(
                !preset.css.contains("url("),
                "{} must work offline",
                preset.id
            );
        }
        assert_eq!(all().len(), 20);
    }
    #[test]
    fn selected_css_round_trips_and_custom_edits_remain_custom() {
        for (index, preset) in all().iter().enumerate() {
            let settings = Settings {
                lyrics_theme_css: preset.css.into(),
                ..Default::default()
            };
            let restored: Settings =
                serde_json::from_str(&serde_json::to_string(&settings).unwrap()).unwrap();
            assert_eq!(index_for(&restored.lyrics_theme_css), index);
            assert_eq!(crate::themes::from_settings(&restored).source, preset.css);
        }
        assert_eq!(
            index_for(".blyrics-container { --blyrics-font-size:31px; }"),
            all().len()
        );
        let mut settings = Settings {
            lyrics_theme_css: all()[3].css.into(),
            lyrics_font_family: "Noto Sans".into(),
            ..Default::default()
        };
        assert_eq!(
            crate::themes::from_settings(&settings).appearance.family,
            "Noto Sans"
        );
        settings.lyrics_font_family.clear();
        assert_eq!(
            crate::themes::from_settings(&settings).appearance.family,
            "Satoshi"
        );
    }
}

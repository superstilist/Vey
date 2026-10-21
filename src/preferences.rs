//! Shared persisted UI preferences, used by native and browser controllers.
use crate::{
    services::{KawarpSettings, Settings},
    CoverBadges, I18n, MainWindow, Motion,
};
use slint::ComponentHandle;

pub fn apply(w: &MainWindow, settings: &Settings) {
    w.set_lyrics_romanization(settings.lyrics_romanization);
    w.set_lyrics_translation(settings.lyrics_translation);
    w.set_lyrics_translation_language(settings.lyrics_translation_language.clone().into());
    let badges = w.global::<CoverBadges>();
    badges.set_shown(settings.download_badge_shown);
    badges.set_style(
        if settings.download_badge_style == "icon" {
            "icon"
        } else {
            "triangle"
        }
        .into(),
    );
    badges.set_position(
        match settings.download_badge_position.as_str() {
            "top-left" => "top-left",
            "top-right" => "top-right",
            "bottom-left" => "bottom-left",
            _ => "bottom-right",
        }
        .into(),
    );
    crate::i18n::set_current(&settings.language);
    w.global::<I18n>()
        .set_language(crate::i18n::language(&settings.language).into());
    let k = settings.kawarp.clone().normalized();
    w.set_kawarp_opacity(k.opacity);
    w.set_kawarp_warp(k.warp);
    w.set_kawarp_blur(k.blur as f32);
    w.set_kawarp_saturation(k.saturation);
    w.set_kawarp_dithering(k.dithering);
    w.set_kawarp_dim(k.dim_bright);
    w.global::<Motion>()
        .set_reduced(settings.lyrics_reduced_motion);
}
pub fn capture(w: &MainWindow, settings: &mut Settings) {
    settings.lyrics_romanization = w.get_lyrics_romanization();
    settings.lyrics_translation = w.get_lyrics_translation();
    settings.lyrics_translation_language = w.get_lyrics_translation_language().to_string();
    let badges = w.global::<CoverBadges>();
    settings.download_badge_shown = badges.get_shown();
    settings.download_badge_style = badges.get_style().to_string();
    settings.download_badge_position = badges.get_position().to_string();
    settings.language = crate::i18n::language(&w.global::<I18n>().get_language()).into();
    crate::i18n::set_current(&settings.language);
    settings.kawarp = KawarpSettings {
        opacity: w.get_kawarp_opacity(),
        warp: w.get_kawarp_warp(),
        blur: w.get_kawarp_blur().round() as u32,
        saturation: w.get_kawarp_saturation(),
        dithering: w.get_kawarp_dithering(),
        dim_bright: w.get_kawarp_dim(),
    }
    .normalized();
}

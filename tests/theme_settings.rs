use playervf::services::Settings;

#[test]
fn language_and_ambient_defaults_and_backups_are_compatible() {
    use playervf::services::KawarpSettings;
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.language, "uk");
    assert_eq!(old.kawarp, KawarpSettings::default());
    for language in playervf::i18n::LANGUAGES {
        let settings = Settings {
            language: language.into(),
            kawarp: KawarpSettings {
                opacity: 0.45,
                dim_bright: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let restored: Settings =
            serde_json::from_value(serde_json::to_value(&settings).unwrap()).unwrap();
        assert_eq!(restored.language, language);
        assert_eq!(restored.kawarp, settings.kawarp);
    }
    let bad = KawarpSettings {
        opacity: f32::NAN,
        blur: 0,
        warp: 9.,
        saturation: -3.,
        dithering: f32::INFINITY,
        ..Default::default()
    }
    .normalized();
    assert_eq!(bad.opacity, 0.28);
    assert_eq!(bad.blur, 1);
    assert_eq!(bad.warp, 1.);
    assert_eq!(bad.saturation, 0.);
    assert_eq!(bad.dithering, 0.02);
}

#[test]
fn old_theme_preferences_do_not_override_playback_or_return_in_backups() {
    for theme in ["Власна", "Караоке", "Контрастна", "PlayerVF"] {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "lyrics_theme": theme, "volume": 0.4, "rate": 1.25,
            "lyrics_reduced_motion": true
        }))
        .unwrap();
        assert_eq!(settings.volume, 0.4);
        assert_eq!(settings.rate, 1.);
        assert!(settings.lyrics_reduced_motion);
        assert!(serde_json::to_value(settings)
            .unwrap()
            .get("lyrics_theme")
            .is_none());
    }
}

#[test]
fn appearance_defaults_and_backup_roundtrip_are_compatible() {
    use playervf::services::ColorMode;
    let old: Settings = serde_json::from_str("{}").unwrap();
    assert_eq!(old.color_mode, ColorMode::Dynamic);
    assert_eq!(old.color_hue, 85);
    for mode in [ColorMode::Dynamic, ColorMode::Static, ColorMode::Background] {
        let settings = Settings {
            color_mode: mode,
            color_hue: 275,
            ..Default::default()
        };
        let restored: Settings =
            serde_json::from_value(serde_json::to_value(settings).unwrap()).unwrap();
        assert_eq!(restored.color_mode, mode);
        assert_eq!(restored.color_hue, 275);
    }
    assert_eq!(
        serde_json::from_str::<Settings>(r#"{"color_mode":"future-mode"}"#)
            .unwrap()
            .color_mode,
        ColorMode::Dynamic
    );
}

#[test]
fn removed_speed_preference_does_not_leave_old_sessions_at_a_hidden_rate() {
    for rate in [0.5, 1.25, 2.] {
        let settings: Settings =
            serde_json::from_value(serde_json::json!({"rate": rate, "volume": 0.4})).unwrap();
        assert_eq!(settings.rate, 1.);
        assert_eq!(settings.volume, 0.4);
        assert!(serde_json::to_value(settings)
            .unwrap()
            .get("rate")
            .is_none());
    }
}

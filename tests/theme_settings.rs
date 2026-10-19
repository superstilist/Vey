use playervf::services::Settings;

#[test]
fn old_theme_preferences_do_not_override_playback_or_return_in_backups() {
    for theme in ["Власна", "Караоке", "Контрастна", "PlayerVF"] {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "lyrics_theme": theme, "volume": 0.4, "rate": 1.25,
            "lyrics_reduced_motion": true
        })).unwrap();
        assert_eq!(settings.volume, 0.4);
        assert_eq!(settings.rate, 1.25);
        assert!(settings.lyrics_reduced_motion);
        assert!(serde_json::to_value(settings).unwrap().get("lyrics_theme").is_none());
    }
}

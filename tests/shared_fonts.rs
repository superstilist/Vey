//! The UI and lyric shaper must use the same complete bundled font bytes.
#[test]
fn generated_ui_exports_the_original_fonts() {
    for (name, shared, original) in [
        (
            "Satoshi 500",
            vey_ui::FONT_SATOSHI_500,
            include_bytes!("../assets/fonts/Satoshi-500.otf").as_slice(),
        ),
        (
            "Satoshi 700",
            vey_ui::FONT_SATOSHI_700,
            include_bytes!("../assets/fonts/Satoshi-700.otf").as_slice(),
        ),
        (
            "Roboto",
            vey_ui::FONT_ROBOTO,
            include_bytes!("../assets/fonts/Roboto.ttf").as_slice(),
        ),
        (
            "Noto Sans",
            vey_ui::FONT_NOTO_SANS,
            include_bytes!("../assets/fonts/NotoSans.ttf").as_slice(),
        ),
        (
            "Noto Sans JP",
            vey_ui::FONT_NOTO_SANS_JP,
            include_bytes!("../assets/fonts/NotoSansJP.ttf").as_slice(),
        ),
    ] {
        assert!(shared == original, "bundled font differs: {name}");
    }
    assert!(std::ptr::eq(
        playervf::FONT_NOTO_SANS_JP,
        vey_ui::FONT_NOTO_SANS_JP
    ));
}

//! Build a browser fixture using the same parser/document adapter as the player.
fn main() {
    let (lyrics, format, sync_type) =
        playervf::lyrics::parse_payload(include_str!("../tests/fixtures/syllables.ttml")).unwrap();
    let alternative = playervf::lyrics::LyricsAlternative {
        provider: "syllable-fixture".into(),
        format: format.into(),
        sync_type,
        lyrics,
    };
    let document = playervf::braccato::web_document(&alternative, 5000);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/lyric-syllables.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, document).unwrap();
    println!("{}", path.display());
}

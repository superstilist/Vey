use std::{collections::HashMap, path::PathBuf};

fn main() {
    let root = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let config = slint_build::CompilerConfiguration::new().with_library_paths(HashMap::from([(
        "braccato".into(),
        root.join("third-party/braccato-rust/crates/slint/ui"),
    )]));
    slint_build::compile_with_config(root.join("ui/main.slint"), config)
        .expect("failed to compile Slint UI");
    export_bundled_fonts(&root);
}

/// Expose Slint's existing embedded font bytes to native lyric shaping. This
/// avoids embedding a second copy of each font in the player library. Slint's
/// generated Rust is version-specific, so fail the build if its format changes.
fn export_bundled_fonts(root: &std::path::Path) {
    let path = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("main.rs");
    let mut generated = std::fs::read_to_string(&path).expect("read generated Slint UI");
    let fonts = [
        ("Satoshi-500.otf", "FONT_SATOSHI_500"),
        ("Satoshi-700.otf", "FONT_SATOSHI_700"),
        ("Roboto.ttf", "FONT_ROBOTO"),
        ("NotoSans.ttf", "FONT_NOTO_SANS"),
        ("NotoSansJP.ttf", "FONT_NOTO_SANS_JP"),
    ];
    // Slint's opt-out uses font paths instead of embedded resources. Keep
    // native shaping functional in this development mode with one own copy.
    if std::env::var("SLINT_EMBED_RESOURCES").as_deref() == Ok("false") {
        for (file, alias) in fonts {
            let font = root.join("assets/fonts").join(file);
            generated.push_str(&format!(
                "\npub static {alias}: &[u8] = include_bytes!({font:?});\n"
            ));
        }
        std::fs::write(path, generated).expect("export fallback font resources");
        return;
    }
    assert!(
        generated.contains("mod slint_generatedMainWindow"),
        "unexpected Slint module name"
    );
    let mut exports = Vec::new();
    for (file, alias) in fonts {
        let marker = format!("{file}\")");
        let mut hits = generated.match_indices(&marker).filter(|(position, _)| {
            let start = generated[..*position].rfind('\n').map_or(0, |i| i + 1);
            let end = generated[*position..]
                .find('\n')
                .map_or(generated.len(), |i| *position + i);
            let line = &generated[start..end];
            line.trim_start()
                .starts_with("static SLINT_EMBEDDED_RESOURCE_")
                && line.contains("include_bytes")
        });
        let position = hits
            .next()
            .unwrap_or_else(|| panic!("missing embedded font {file}"))
            .0;
        assert!(hits.next().is_none(), "ambiguous embedded font {file}");
        let start = generated[..position].rfind('\n').map_or(0, |i| i + 1);
        let end = generated[position..]
            .find('\n')
            .map_or(generated.len(), |i| position + i);
        let line = &generated[start..end];
        let declaration = line.trim_start();
        assert!(
            declaration.starts_with("static SLINT_EMBEDDED_RESOURCE_")
                && line.contains("include_bytes"),
            "unexpected font declaration for {file}"
        );
        let name = declaration
            .split_whitespace()
            .nth(1)
            .expect("font static name")
            .to_owned();
        let offset = start + (line.len() - declaration.len());
        generated.insert_str(offset, "pub ");
        exports.push((name, alias));
    }
    for (name, alias) in exports {
        generated.push_str(&format!(
            "\npub use self::slint_generatedMainWindow::{name} as {alias};\n"
        ));
    }
    std::fs::write(path, generated).expect("export Slint font resources");
}

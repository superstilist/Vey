fn main() {
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=ui/controls.slint");
    println!("cargo:rerun-if-changed=ui/tokens.slint");
    println!("cargo:rerun-if-changed=ui/braccato-lyrics.slint");
    println!("cargo:rerun-if-changed=ui/lyric-types.slint");
    slint_build::compile("ui/main.slint").expect("failed to compile Slint UI");
}

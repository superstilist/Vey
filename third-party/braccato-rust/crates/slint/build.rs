fn main() {
    println!("cargo:rerun-if-changed=ui/lyrics.slint");
    slint_build::compile("ui/lyrics.slint").expect("compile reusable lyrics component");
}

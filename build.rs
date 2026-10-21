fn main() {
    println!("cargo:rerun-if-changed=android/java/app/vey/VeyBridge.java");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        android_bridge();
    }
    println!("cargo:rerun-if-changed=ui/main.slint");
    println!("cargo:rerun-if-changed=ui/controls.slint");
    println!("cargo:rerun-if-changed=ui/tokens.slint");
    println!("cargo:rerun-if-changed=ui/braccato-lyrics.slint");
    println!("cargo:rerun-if-changed=ui/lyric-types.slint");
    slint_build::compile("ui/main.slint").expect("failed to compile Slint UI");
}

fn android_bridge() {
    use std::{path::PathBuf, process::Command};
    let sdk = PathBuf::from(
        std::env::var_os("ANDROID_HOME")
            .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
            .expect("Android SDK is required"),
    );
    let java = PathBuf::from(std::env::var_os("JAVA_HOME").expect("JDK 17+ is required"));
    let newest = |dir: PathBuf| {
        let mut paths: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        paths.sort_by_key(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .split(|c: char| !c.is_ascii_digit())
                .filter_map(|s| s.parse::<u32>().ok())
                .collect::<Vec<_>>()
        });
        paths.pop().expect("Android SDK components are missing")
    };
    let platform = newest(sdk.join("platforms")).join("android.jar");
    let d8 = newest(sdk.join("build-tools")).join("lib/d8.jar");
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("android-bridge");
    let classes = out.join("java");
    std::fs::create_dir_all(&classes).unwrap();
    let exe = |name: &str| {
        java.join("bin").join(if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.into()
        })
    };
    assert!(
        Command::new(exe("javac"))
            .args([
                "-source",
                "8",
                "-target",
                "8",
                "-encoding",
                "UTF-8",
                "-classpath"
            ])
            .arg(&platform)
            .arg("-d")
            .arg(&classes)
            .arg("android/java/app/vey/VeyBridge.java")
            .status()
            .unwrap()
            .success(),
        "Android bridge javac failed"
    );
    let files: Vec<_> = std::fs::read_dir(classes.join("app/vey"))
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "class"))
        .collect();
    assert!(
        Command::new(exe("java"))
            .arg("-cp")
            .arg(d8)
            .arg("com.android.tools.r8.D8")
            .arg("--min-api")
            .arg("26")
            .arg("--lib")
            .arg(platform)
            .arg("--output")
            .arg(&out)
            .args(files)
            .status()
            .unwrap()
            .success(),
        "Android bridge dex compilation failed"
    );
}

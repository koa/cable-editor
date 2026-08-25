use std::{env, path::Path, process::Command};

fn main() {
    let frontend_dir = Path::new("../cable-editor-frontend");
    let frontend_target_dir = frontend_dir.join("target");
    let profile = env::var("PROFILE").unwrap();

    // 2. Führe 'trunk build' im Frontend-Verzeichnis aus
    let mut cmd = Command::new("trunk");
    cmd.arg("build")
        .current_dir(frontend_dir)
        .env("CARGO_TARGET_DIR", frontend_target_dir);
    if profile == "release" {
        cmd.arg("--release");
    }
    let status = cmd
        .status()
        .expect("Fehler: Konnte 'trunk' nicht ausführen. Ist Trunk installiert?");

    if !status.success() {
        panic!("Trunk Build ist fehlgeschlagen!");
    }

    // 3. (Wichtig für Dev-Modus): Sag Cargo, wann es dieses Skript neu ausführen soll.
    // Ansonsten würde Trunk bei jedem noch so kleinen Backend-Build neu anlaufen.
    println!("cargo:rerun-if-changed=../cable-editor-frontend/src");
    println!("cargo:rerun-if-changed=../cable-editor-frontend/index.html");
    println!("cargo:rerun-if-changed=../cable-editor-frontend/Trunk.toml");
}

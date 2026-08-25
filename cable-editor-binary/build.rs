use std::{path::Path, process::Command};

fn main() {
    let frontend_dir = Path::new("../cable-editor-frontend");

    // 2. Führe 'trunk build' im Frontend-Verzeichnis aus
    let status = Command::new("trunk")
        .arg("build")
        .arg("--release") // Optional: Für Production optimieren
        .current_dir(frontend_dir)
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

use std::path::Path;
use std::process::Command;

fn msgfmt_available() -> bool {
    Command::new("msgfmt").arg("--version").output().is_ok()
}

fn main() {
    let po_dir = Path::new("data/po");
    if !po_dir.exists() {
        return;
    }

    if !msgfmt_available() {
        println!("cargo:rerun-if-changed=data/po");
        return;
    }

    let entries = match std::fs::read_dir(po_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map(|e| e == "po").unwrap_or(false) {
            let lang = match path.file_stem().and_then(|s| s.to_str()) {
                Some(l) => l,
                None => continue,
            };
            let out_dir = po_dir.join(lang).join("LC_MESSAGES");
            if let Err(e) = std::fs::create_dir_all(&out_dir) {
                println!(
                    "cargo:warning=Failed to create locale dir for {}: {}",
                    lang, e
                );
                continue;
            }
            let mo_path = out_dir.join("pot-gtk.mo");

            let status = Command::new("msgfmt")
                .arg(&path)
                .arg("-o")
                .arg(&mo_path)
                .status();

            match status {
                Ok(s) if s.success() => {}
                Ok(s) => println!("cargo:warning=msgfmt failed for {} (exit {})", lang, s),
                Err(e) => println!("cargo:warning=Failed to run msgfmt for {}: {}", lang, e),
            }
        }
    }

    println!("cargo:rerun-if-changed=data/po");
}

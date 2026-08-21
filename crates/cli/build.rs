//! Keeps `ui/dist` in existence and fresh before `rust-embed` captures
//! it: when the frontend sources are newer than the bundle, run the bun
//! build so the embedded UI always matches the checked-out code.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

fn main() {
    let ui = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../ui");
    let ui = ui.canonicalize().unwrap_or(ui);

    let mut sources = vec![
        ui.join("package.json"),
        ui.join("bunfig.toml"),
        ui.join("build.ts"),
        ui.join("bun.lock"),
    ];
    collect_files(&ui.join("src"), &mut sources);
    for source in &sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }
    // Watching the directory itself catches added or removed files, and
    // watching the bundle entry catches a deleted dist/.
    println!("cargo:rerun-if-changed={}", ui.join("src").display());
    let dist_index = ui.join("dist/index.html");
    println!("cargo:rerun-if-changed={}", dist_index.display());

    let newest_source = sources.iter().filter_map(|p| mtime(p)).max();
    let fresh = match (mtime(&dist_index), newest_source) {
        (Some(dist), Some(source)) => dist >= source,
        (Some(_), None) => true,
        (None, _) => false,
    };
    if fresh {
        return;
    }

    if !ui.join("node_modules").is_dir() {
        bun(&ui, &["install", "--ignore-scripts"]);
    }
    bun(&ui, &["run", "build"]);
    assert!(
        dist_index.is_file(),
        "`bun run build` did not produce ui/dist/index.html"
    );
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

fn bun(ui: &Path, args: &[&str]) {
    let status = Command::new("bun").args(args).current_dir(ui).status();
    match status {
        Ok(s) if s.success() => {}
        Ok(s) => panic!(
            "`bun {}` in {} failed with {s}",
            args.join(" "),
            ui.display()
        ),
        Err(e) => panic!(
            "cannot run `bun {}` ({e}); install bun or build ui/dist by hand",
            args.join(" ")
        ),
    }
}

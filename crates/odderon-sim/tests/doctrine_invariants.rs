use std::fs;
use std::path::Path;

const FORBIDDEN_ASSET_EXTS: &[&str] = &[
    "glb", "gltf", "obj", "fbx", "stl", "dae", "blend",
    "png", "jpg", "jpeg", "webp", "ktx2", "dds", "tga",
    "mp3", "wav", "ogg", "flac", "aac", "m4a",
];

fn scan_dir_for_assets(dir: &Path, offenders: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        let name = entry.file_name();
        let s = name.to_string_lossy();
        if s.starts_with('.') || s == "target" || s == "node_modules" {
            continue;
        }
        if p.is_dir() {
            scan_dir_for_assets(&p, offenders);
        } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            let low = ext.to_ascii_lowercase();
            if FORBIDDEN_ASSET_EXTS.contains(&low.as_str()) {
                offenders.push(p.display().to_string());
            }
        }
    }
}

#[test]
fn law_5_zero_binary_3d_texture_or_audio_assets_in_repo() {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let mut offenders = Vec::new();
    scan_dir_for_assets(repo_root, &mut offenders);
    assert!(
        offenders.is_empty(),
        "Law 5 violated! Found pre-made asset files instead of parametric math/synthesis: {:?}",
        offenders
    );
}

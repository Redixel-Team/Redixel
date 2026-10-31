//! Internal build tool: collects each example's `[package.metadata.game]` into the web playground's `games_manifest.json`.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use toml::de;

/// One example's public metadata, serialized to `dist/games_manifest.json`.
#[derive(Serialize)]
struct GameManifest {
    id: String,
    title: String,
    description: String,
    tags: Vec<String>,
}

/// Mirrors the `[package]` / `[package.metadata.game]` tables of an example's
/// `Cargo.toml`, just deep enough to read the fields `GameManifest` needs.
#[derive(Deserialize)]
struct CargoManifest {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    metadata: Option<Metadata>,
}

#[derive(Deserialize)]
struct Metadata {
    game: Option<GameMetadata>,
}

#[derive(Deserialize)]
struct GameMetadata {
    title: String,
    description: String,
    #[serde(default)]
    tags: Vec<String>,
}

/// Scans `examples/*/Cargo.toml` for a `[package.metadata.game]` table and
/// writes the matches to `dist/games_manifest.json`. Examples without that
/// table (internal/dev-only ones) are skipped.
fn main() {
    let examples_dir: &Path = Path::new("examples");
    let mut games: Vec<GameManifest> = Vec::new();

    let mut entries: Vec<fs::DirEntry> = fs::read_dir(examples_dir)
        .expect("examples/ directory not found")
        .filter_map(|e: Result<fs::DirEntry, io::Error>| e.ok())
        .filter(|e: &fs::DirEntry| e.file_type().map(|t: fs::FileType| t.is_dir()).unwrap_or(false))
        .collect();

    entries.sort_by_key(|e: &fs::DirEntry| e.file_name());

    for entry in entries {
        let manifest_path: PathBuf = entry.path().join("Cargo.toml");
        let Ok(content): Result<String, io::Error> = fs::read_to_string(&manifest_path) else {
            continue;
        };

        // A manifest that fails to parse would silently drop its game from the playground.
        let parsed: Result<CargoManifest, de::Error> = toml::from_str(&content);
        let manifest: CargoManifest = match parsed {
            Ok(manifest) => manifest,
            Err(e) => {
                eprintln!("builder: failed to parse {}: {e}", manifest_path.display());
                std::process::exit(1);
            }
        };

        let Some(game): Option<GameMetadata> = manifest.package.metadata.and_then(|m: Metadata| m.game) else {
            continue;
        };

        games.push(GameManifest {
            id: manifest.package.name,
            title: game.title,
            description: game.description,
            tags: game.tags,
        });
    }

    let dist_dir: &Path = Path::new("dist");
    if !dist_dir.exists() {
        fs::create_dir_all(dist_dir).expect("failed to create dist/");
    }

    let json: String = serde_json::to_string_pretty(&games).expect("failed to serialize manifest");
    fs::write(dist_dir.join("games_manifest.json"), json).expect("failed to write games_manifest.json");
}

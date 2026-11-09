// SPDX-License-Identifier: MIT OR Apache-2.0

//! Generates the table of the files in `assets/`, which
//! `src/delivery/assets.rs` embeds into the binary.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// The directory with the assets, relative to the package root.
const ASSETS: &str = "assets";

/// Generates the table in `$OUT_DIR/assets.rs`.
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed={ASSETS}");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("cargo should set OUT_DIR")?);
    let mut files = Vec::new();
    collect_dir(Path::new(ASSETS), "", &mut files)?;
    Ok(write_table(&out_dir, files)?)
}

/// Adds the files in `source` and its subdirectories to `files`. The names
/// are relative to the assets directory and start with `prefix`.
fn collect_dir(
    source: &Path,
    prefix: &str,
    files: &mut Vec<(String, &'static str)>,
) -> Result<(), Box<dyn Error>> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = format!("{prefix}{}", entry.file_name().display());
        if entry.file_type()?.is_dir() {
            collect_dir(&path, &format!("{name}/"), files)?;
        } else {
            files.push((name, content_type(&path)?));
        }
    }
    Ok(())
}

/// Returns the `Content-Type` of the asset at `path`.
fn content_type(path: &Path) -> Result<&'static str, Box<dyn Error>> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("css") => Ok("text/css; charset=utf-8"),
        Some("js") => Ok("text/javascript; charset=utf-8"),
        Some("png") => Ok("image/png"),
        Some("webp") => Ok("image/webp"),
        _ => Err(format!("{}: unknown file type", path.display()).into()),
    }
}

/// Writes the table of `files`, as `(name, content type)`, that
/// `src/delivery/assets.rs` includes.
fn write_table(out_dir: &Path, mut files: Vec<(String, &str)>) -> io::Result<()> {
    // `read_dir` has no fixed order, and the build should be reproducible.
    files.sort_unstable();
    let rows: String = files
        .iter()
        .map(|(name, content_type)| {
            format!(
                "({name:?}, {content_type:?}, \
                 include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/assets/{name}\"))),\n"
            )
        })
        .collect();
    fs::write(out_dir.join("assets.rs"), format!("&[\n{rows}]"))
}

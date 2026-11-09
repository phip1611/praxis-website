// SPDX-License-Identifier: MIT OR Apache-2.0

//! Prepares the files in `assets/` for embedding into the binary: minifies
//! the stylesheet and the scripts, and copies everything else. Also generates
//! the table of the files. `src/delivery/assets.rs` embeds the results from
//! `OUT_DIR`.
//!
//! Brotli already compresses whitespace and comments well, but minifying
//! still roughly halves their compressed size.

use lightningcss::stylesheet::{MinifyOptions, ParserOptions, PrinterOptions, StyleSheet};
use oxc::allocator::Allocator;
use oxc::codegen::{Codegen, CodegenOptions};
use oxc::minifier::{Minifier, MinifierOptions};
use oxc::parser::Parser;
use oxc::span::SourceType;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::{env, fs, io};

/// The directory with the files to prepare, relative to the package root.
const ASSETS: &str = "assets";

/// Prepares all assets into `$OUT_DIR/assets` and generates their table in
/// `$OUT_DIR/assets.rs`.
fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed={ASSETS}");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").ok_or("cargo should set OUT_DIR")?);
    let mut files = Vec::new();
    prepare_dir(Path::new(ASSETS), &out_dir.join(ASSETS), "", &mut files)?;
    Ok(write_table(&out_dir, files)?)
}

/// Prepares the files in `source` and its subdirectories into `target`, and
/// adds them to `files`. The names are relative to the assets directory and
/// start with `prefix`.
fn prepare_dir(
    source: &Path,
    target: &Path,
    prefix: &str,
    files: &mut Vec<(String, &'static str)>,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let name = format!("{prefix}{}", entry.file_name().display());
        let target = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            prepare_dir(&path, &target, &format!("{name}/"), files)?;
            continue;
        }
        let bytes = match path.extension().and_then(|extension| extension.to_str()) {
            Some("css") => minify_css(&fs::read_to_string(&path)?),
            Some("js") => minify_js(&fs::read_to_string(&path)?),
            _ => Ok(fs::read(&path)?),
        }
        .map_err(|err| format!("{}: {err}", path.display()))?;
        fs::write(target, bytes)?;
        files.push((name, content_type(&path)?));
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
                 include_bytes!(concat!(env!(\"OUT_DIR\"), \"/assets/{name}\"))),\n"
            )
        })
        .collect();
    fs::write(out_dir.join("assets.rs"), format!("&[\n{rows}]"))
}

/// Minifies a stylesheet. Without browser targets, modern syntax stays as it
/// is, so the result needs the same browsers as the source.
fn minify_css(source: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut stylesheet =
        StyleSheet::parse(source, ParserOptions::default()).map_err(|err| err.to_string())?;
    stylesheet.minify(MinifyOptions::default())?;
    let printed = stylesheet.to_css(PrinterOptions {
        minify: true,
        ..PrinterOptions::default()
    })?;
    Ok(printed.code.into_bytes())
}

/// Minifies a classic script, i.e., not a module. Only local names are
/// shortened, as the scripts wrap their code in a function anyway.
fn minify_js(source: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, SourceType::script()).parse();
    if let Some(err) = parsed.diagnostics.first() {
        return Err(err.to_string().into());
    }
    let mut program = parsed.program;
    let minified = Minifier::new(MinifierOptions::default()).minify(&allocator, &mut program);
    let printed = Codegen::new()
        .with_options(CodegenOptions::minify())
        .with_scoping(minified.scoping)
        .with_private_member_mappings(minified.class_private_mappings)
        .build(&program);
    Ok(printed.code.into_bytes())
}

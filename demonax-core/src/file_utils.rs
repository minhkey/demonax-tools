//! File utility functions.

use crate::error::{DemonaxError, Result};
use std::path::Path;
use walkdir::WalkDir;

/// Recursively find files with given extension in a directory.
pub fn find_files_with_extension(dir: &Path, extension: &str) -> Result<Vec<std::path::PathBuf>> {
    if !dir.exists() {
        return Err(DemonaxError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Directory not found: {}", dir.display()),
        )));
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == extension {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }
    Ok(files)
}

/// Read file with Latin1 encoding (Windows-1252).
pub fn read_latin1_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let (text, _, had_errors) = encoding_rs::WINDOWS_1252.decode(&bytes);
    if had_errors {
        return Err(DemonaxError::Parse("Failed to decode Latin1 text".to_string()));
    }
    Ok(text.into_owned())
}

/// Read file with UTF-8 encoding.
pub fn read_utf8_file(path: &Path) -> Result<String> {
    Ok(std::fs::read_to_string(path)?)
}
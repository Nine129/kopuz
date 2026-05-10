use super::collage::generate_album_collages;
use super::metadata::read;
use super::models::Library;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fs;

/// Collect all new audio files from `dir` and its subdirectories (recursive, iterative)
/// that are not already tracked in `existing_paths`.
///
/// Uses a worklist instead of recursion to avoid deep call stacks on deeply nested
/// directory trees, and returns every file in one batch so the caller can process
/// them in a single blocking task.
fn collect_new_audio_files(root: &Path, existing_paths: &HashSet<PathBuf>) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut dirs_to_scan = vec![root.to_path_buf()];

    while let Some(dir) = dirs_to_scan.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs_to_scan.push(path);
            } else if is_audio_file(&path) && !existing_paths.contains(&path) {
                result.push(path);
            }
        }
    }

    result
}

pub async fn scan_directory(
    dir: PathBuf,
    cover_cache: PathBuf,
    library: &mut Library,
    on_progress: Arc<dyn Fn(String) + Send + Sync>,
) -> std::io::Result<()> {
    tracing::info!("[scanner] scan_directory called for {:?}", dir);
    tracing::info!("[scanner] library has {} tracks, {} albums", library.tracks.len(), library.albums.len());
    let existing_paths: HashSet<PathBuf> = library.tracks.iter().map(|t| t.path.clone()).collect();
    tracing::info!("[scanner] {} existing paths, starting scan", existing_paths.len());

    // Collect all new audio files in one pass (iterative worklist, not recursion),
    // then process them in a single blocking task. This avoids N spawn_blocking
    // calls for deeply nested directory trees.
    let all_audio_files = collect_new_audio_files(&dir, &existing_paths);

    tracing::info!("[scanner] found {} new audio files (recursive), starting processing", all_audio_files.len());

    if !all_audio_files.is_empty() {
        let mut lib = std::mem::take(library);
        let cover_cache_clone = cover_cache.clone();
        let progress = on_progress.clone();

        lib = tokio::task::spawn_blocking(move || {
            tracing::info!("[scanner] processing {} audio files in blocking task", all_audio_files.len());
            for path in all_audio_files {
                if let Some(name) = path.file_name() {
                    progress(name.to_string_lossy().into_owned());
                }
                read(&path, &cover_cache_clone, &mut lib);
            }
            tracing::info!("[scanner] done processing, library now has {} tracks", lib.tracks.len());
            lib
        })
        .await
        .unwrap();

        // Generate collage covers for albums that lack a folder cover
        let cover_cache_for_collage = cover_cache.clone();
        lib = tokio::task::spawn_blocking(move || {
            generate_album_collages(&mut lib, &cover_cache_for_collage);
            lib
        })
        .await
        .unwrap();

        *library = lib;
    }

    Ok(())
}

pub fn is_audio_file(path: &Path) -> bool {
    let extensions = ["mp3", "flac", "m4a", "wav", "ogg", "opus", "mp4"];
    path.extension()
        .and_then(|s| s.to_str())
        .map(|s| extensions.contains(&s.to_lowercase().as_str()))
        .unwrap_or(false)
}

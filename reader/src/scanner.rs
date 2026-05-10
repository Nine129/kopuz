use super::metadata::read;
use super::models::Library;
use async_recursion::async_recursion;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::fs;

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
    scan_directory_internal(dir, cover_cache, library, &existing_paths, on_progress).await
}

#[async_recursion]
async fn scan_directory_internal(
    dir: PathBuf,
    cover_cache: PathBuf,
    library: &mut Library,
    existing_paths: &HashSet<PathBuf>,
    on_progress: Arc<dyn Fn(String) + Send + Sync>,
) -> std::io::Result<()> {
    let mut audio_files = Vec::new();
    let mut sub_dirs = Vec::new();

    let entries = match fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            sub_dirs.push(path);
        } else if is_audio_file(&path) {
            if !existing_paths.contains(&path) {
                audio_files.push(path);
            }
        }
    }

    tracing::info!("[scanner] found {} audio files, {} subdirs in {:?}", audio_files.len(), sub_dirs.len(), dir);
    if !audio_files.is_empty() {
        let mut lib = std::mem::take(library);
        let cover_cache_clone = cover_cache.clone();
        let progress = on_progress.clone();

        lib = tokio::task::spawn_blocking(move || {
            tracing::info!("[scanner] processing {} audio files in blocking task", audio_files.len());
            for path in audio_files {
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

        *library = lib;
    }

    for sub_dir in sub_dirs {
        let _ = scan_directory_internal(
            sub_dir,
            cover_cache.clone(),
            library,
            existing_paths,
            on_progress.clone(),
        )
        .await;
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

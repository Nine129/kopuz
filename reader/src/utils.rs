use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::models::Library;

pub fn find_folder_cover(dir: &Path) -> Option<PathBuf> {
    let candidates = ["cover.jpg", "cover.png", "folder.jpg", "album.jpg"];

    for name in candidates {
        let p = dir.join(name);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

pub fn save_cover(album_id: &str, data: &[u8], cache_dir: &Path) -> std::io::Result<PathBuf> {
    fs::create_dir_all(cache_dir)?;
    let path = cache_dir.join(format!("{album_id}.jpg"));
    let bytes = data.to_vec();

    fs::write(&path, bytes)?;
    Ok(path)
}

/// Remove cover files from the cache that are no longer referenced by any track or album.
///
/// After a full library scan, orphaned cover files accumulate when tracks are removed
/// or replaced. This function walks the cover cache directory and deletes any `.jpg` file
/// whose path is not present in the active library's `cover_path` fields.
pub fn prune_cover_cache(library: &Library, cover_cache: &Path) -> std::io::Result<()> {
    if !cover_cache.exists() {
        return Ok(());
    }

    // Build a set of all cover paths currently referenced by the library
    let mut active_covers: HashSet<PathBuf> = HashSet::new();

    for album in &library.albums {
        if let Some(ref path) = album.cover_path {
            if path.starts_with(cover_cache) {
                active_covers.insert(path.clone());
            }
        }
    }

    for track in &library.tracks {
        if let Some(ref path) = track.cover_path {
            if path.starts_with(cover_cache) {
                active_covers.insert(path.clone());
            }
        }
    }

    // Delete orphaned cache files
    for entry in fs::read_dir(cover_cache)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file()
            && path.extension().map(|e| e == "jpg").unwrap_or(false)
            && !active_covers.contains(&path)
        {
            let _ = fs::remove_file(&path);
        }
    }

    Ok(())
}

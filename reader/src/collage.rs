use crate::models::Library;
use image::imageops::FilterType;
use image::GenericImage;
use image::RgbImage;
use rand::seq::SliceRandom;
use std::path::{Path, PathBuf};

/// Output thumbnail size (square).
const OUTPUT_SIZE: u32 = 600;
/// Tile size for 2×2 grid cells.
const TILE: u32 = 300;

/// For every album in the library that lacks a cover (`cover_path` is `None`),
/// generate a 2×2 grid collage from up to 4 randomly chosen track covers.
///
/// Always produces a full 2×2 grid — if fewer than 4 unique covers are
/// available, repeats them cyclically to fill all four cells.
///
/// Albums that already have a cover (e.g. from a `folder.jpg`) are left untouched.
pub fn generate_album_collages(library: &mut Library, cover_cache: &Path) {
    let mut tracks_by_album: std::collections::HashMap<&str, Vec<&PathBuf>> =
        std::collections::HashMap::new();
    for track in &library.tracks {
        if let Some(ref cover) = track.cover_path {
            tracks_by_album
                .entry(track.album_id.as_str())
                .or_default()
                .push(cover);
        }
    }

    for album in &mut library.albums {
        if album.cover_path.is_some() {
            continue;
        }

        let Some(covers) = tracks_by_album.get(album.id.as_str()) else {
            continue;
        };
        if covers.is_empty() {
            continue;
        }

        let chosen: Vec<PathBuf> = covers
            .choose_multiple(&mut rand::thread_rng(), 4)
            .copied()
            .cloned()
            .collect();

        if chosen.is_empty() {
            continue;
        }

        tracing::debug!(
            "[collage] album {} has {} covers available",
            album.id,
            chosen.len()
        );

        if let Some(path) = create_collage(&chosen, &album.id, cover_cache) {
            album.cover_path = Some(path);
        }
    }
}

fn create_collage(
    cover_paths: &[PathBuf],
    album_id: &str,
    cache_dir: &Path,
) -> Option<PathBuf> {
    let mut unique: Vec<RgbImage> = Vec::with_capacity(cover_paths.len());
    for path in cover_paths {
        if let Ok(img) = image::open(path) {
            unique.push(img.to_rgb8());
            if unique.len() >= 4 {
                break;
            }
        }
    }

    if unique.is_empty() {
        tracing::warn!("[collage] no valid cover images for {}", album_id);
        return None;
    }

    tracing::debug!(
        "[collage] {} loaded {} unique covers",
        album_id,
        unique.len()
    );

    let mut canvas = RgbImage::new(OUTPUT_SIZE, OUTPUT_SIZE);

    macro_rules! place {
        ($src:expr, $w:expr, $h:expr, $x:expr, $y:expr) => {
            let tile = image::imageops::resize($src, $w, $h, FilterType::Lanczos3);
            if let Err(e) = canvas.copy_from(&tile, $x, $y) {
                tracing::error!("[collage] copy_from ({},{}): {}", $x, $y, e);
            }
        };
    }

    match unique.len() {
        1 => {
            // Full-size single cover
            place!(&unique[0], OUTPUT_SIZE, OUTPUT_SIZE, 0, 0);
        }
        2 => {
            // Side-by-side vertical strips
            place!(&unique[0], TILE, OUTPUT_SIZE, 0, 0);
            place!(&unique[1], TILE, OUTPUT_SIZE, TILE, 0);
        }
        3 => {
            // Top row 2 tiles, bottom row 1 centred
            place!(&unique[0], TILE, TILE, 0, 0);
            place!(&unique[1], TILE, TILE, TILE, 0);
            place!(&unique[2], TILE, TILE, TILE / 2, TILE);
        }
        _ => {
            // 2×2 grid
            place!(&unique[0], TILE, TILE, 0, 0);
            place!(&unique[1], TILE, TILE, TILE, 0);
            place!(&unique[2], TILE, TILE, 0, TILE);
            place!(&unique[3], TILE, TILE, TILE, TILE);
        }
    }

    let output_path = cache_dir.join(format!("{album_id}.jpg"));
    match canvas.save(&output_path) {
        Ok(_) => {
            tracing::debug!("[collage] saved {}", output_path.display());
            Some(output_path)
        }
        Err(e) => {
            tracing::error!("[collage] failed to save {}: {}", output_path.display(), e);
            None
        }
    }
}

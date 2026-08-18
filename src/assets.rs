//! Cache-busting version for `/static/` asset URLs.
//!
//! nginx serves `/static/` with `Cache-Control: public, immutable` and a
//! one-year lifetime (`deploy/nginx.conf`), so a browser that has a file
//! never asks for it again — not even a conditional revalidation. That is
//! the point of the header, but it means a deploy that changes `style.css`
//! under the same URL leaves every returning visitor on the old file for up
//! to a year (observed 2026-08-18: a phone rendered the new fitness Today
//! templates against the pre-overhaul stylesheet — dark background, nothing
//! else styled).
//!
//! The fix is to change the URL when the content changes: templates append
//! `?v={{ crate::assets::asset_version() }}` to every CSS/JS link. The
//! version is a hash of the top-level files in `static/`, computed once at
//! startup — the same deploy that ships new assets ships HTML pointing at
//! new URLs, and the immutable header keeps doing its job for everything
//! unchanged.
//!
//! Only the top-level files are hashed: those are the ones templates link
//! directly. `static/fonts/` and friends are referenced from *inside*
//! `style.css`, where this version string cannot reach — and a font preload
//! `<link>` must keep the exact URL the CSS uses, or the browser fetches
//! both. So font/icon URLs stay unversioned by design; changing one means
//! renaming it.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::OnceLock;

static VERSION: OnceLock<String> = OnceLock::new();

/// The version string templates append as `?v=...` to `/static/` URLs.
///
/// Computed on first use and fixed for the life of the process — ServeDir
/// reads `static/` from disk per request, but serving a mid-run edit under
/// the old version would poison year-long caches, so a restart (which every
/// deploy performs) is the unit of change.
pub fn asset_version() -> &'static str {
    VERSION.get_or_init(|| compute(Path::new("static")))
}

/// Hashes the name and content of every top-level file in `dir`, sorted so
/// directory iteration order can't change the result. `DefaultHasher::new()`
/// is keyed with constants, so the value is stable across runs and across
/// the build/deploy boundary.
fn compute(dir: &Path) -> String {
    let mut files: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect(),
        // No static dir (unit tests, odd working directory): a constant
        // version keeps pages rendering; caching just won't bust.
        Err(_) => return "0".to_string(),
    };
    files.sort();

    let mut hasher = DefaultHasher::new();
    for path in &files {
        path.file_name().hash(&mut hasher);
        match std::fs::read(path) {
            Ok(bytes) => bytes.hash(&mut hasher),
            Err(_) => 0u8.hash(&mut hasher),
        }
    }
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("asset-version-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_version_is_deterministic() {
        let dir = temp_dir("det");
        std::fs::write(dir.join("style.css"), "body{}").unwrap();
        std::fs::write(dir.join("app.js"), "let x=1;").unwrap();
        assert_eq!(compute(&dir), compute(&dir));
    }

    #[test]
    fn test_version_changes_when_content_changes() {
        let dir = temp_dir("content");
        std::fs::write(dir.join("style.css"), "body{}").unwrap();
        let before = compute(&dir);
        std::fs::write(dir.join("style.css"), "body{color:red}").unwrap();
        assert_ne!(before, compute(&dir));
    }

    #[test]
    fn test_missing_dir_yields_constant() {
        assert_eq!(compute(Path::new("/nonexistent-asset-dir")), "0");
    }

    #[test]
    fn test_real_static_dir_produces_a_version() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("static");
        let v = compute(&dir);
        assert_ne!(v, "0", "static/ should exist in the repo");
        assert_eq!(v.len(), 16);
    }
}

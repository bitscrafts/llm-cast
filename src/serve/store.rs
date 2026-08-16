//! Live HLS artifact store (R5): what the HTTP server reads from.
//!
//! [`MapStore`] is the in-memory, seedable store used by tests and the
//! default-features dry-run; [`DirStore`] is the production store and reads
//! the `hlssink2` output dir (`live.m3u8` + `segment/seg_*.ts`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// The live artifact store the HLS HTTP server reads from.
pub trait MediaStore: Send + Sync {
    /// Raw `live.m3u8` playlist text, or `None` if not (yet) published.
    fn playlist(&self) -> Option<String>;
    /// One media segment by file name (e.g. `seg_00000.ts`), or `None`.
    fn segment(&self, name: &str) -> Option<Vec<u8>>;
}

/// In-memory, seedable store — tests and the default-features dry-run.
pub struct MapStore {
    map: Mutex<HashMap<String, Vec<u8>>>,
}

impl MapStore {
    /// An empty store.
    pub fn new() -> Self {
        MapStore {
            map: Mutex::new(HashMap::new()),
        }
    }

    /// A store pre-seeded with a playlist (under the name `live.m3u8`) and
    /// one segment, so a fresh server answers 200 for both routes.
    pub fn seeded(playlist: &str, segment_name: &str, segment_bytes: Vec<u8>) -> Self {
        let store = MapStore::new();
        store.insert("live.m3u8", playlist.as_bytes().to_vec());
        store.insert(segment_name, segment_bytes);
        store
    }

    /// Store/replace one artifact.
    pub fn insert(&self, name: &str, bytes: Vec<u8>) {
        if let Ok(mut map) = self.map.lock() {
            map.insert(name.to_string(), bytes);
        }
    }
}

impl Default for MapStore {
    fn default() -> Self {
        MapStore::new()
    }
}

impl MediaStore for MapStore {
    fn playlist(&self) -> Option<String> {
        let map = self.map.lock().ok()?;
        let bytes = map.get("live.m3u8")?;
        String::from_utf8(bytes.clone()).ok()
    }

    fn segment(&self, name: &str) -> Option<Vec<u8>> {
        let map = self.map.lock().ok()?;
        map.get(name).cloned()
    }
}

/// Production store: reads the `hlssink2` output directory.
pub struct DirStore {
    dir: PathBuf,
}

impl DirStore {
    /// A store over the directory holding `live.m3u8` + `segment/`.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        DirStore { dir: dir.into() }
    }
}

impl MediaStore for DirStore {
    fn playlist(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join("live.m3u8")).ok()
    }

    fn segment(&self, name: &str) -> Option<Vec<u8>> {
        // The axum path parameter never carries a '/', but reject traversal
        // forms explicitly — this path is fed by network requests.
        if name.contains('/') || name.contains('\\') || name.contains("..") {
            return None;
        }
        // hlssink2 writes segments into `dir/segment/` (the playlist stays in
        // `dir/`); the store follows that layout.
        std::fs::read(self.dir.join("segment").join(name)).ok()
    }
}

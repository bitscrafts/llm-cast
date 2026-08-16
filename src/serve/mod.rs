//! HLS HTTP serving (R5): playlist + segments with CORS, store-driven.

pub mod server;
pub mod store;

pub use store::{DirStore, MapStore, MediaStore};

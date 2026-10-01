//! `Storage` trait 구현체들.

pub mod fs;
pub mod memory;

pub use fs::{find_local, init_local, FsStorage, InitOutcome};
pub use memory::MemoryStorage;

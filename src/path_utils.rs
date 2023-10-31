use std::path::PathBuf;

use crate::constants::{CACHE_PATH, ORIGINAL_PATH, PENDING_PATH, UNAPPROVED_PATH};

pub fn get_pending_path() -> PathBuf {
    return PENDING_PATH.iter().collect();
}

pub fn get_unapproved_path() -> PathBuf {
    return UNAPPROVED_PATH.iter().collect();
}

pub fn get_original_path() -> PathBuf {
    return ORIGINAL_PATH.iter().collect();
}

pub fn get_cache_path() -> PathBuf {
    return CACHE_PATH.iter().collect();
}

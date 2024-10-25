use std::path::PathBuf;

use crate::constants::{CACHE_PATH, ORIGINAL_PATH, PENDING_PATH, RAW_PATH, UNAPPROVED_PATH};

// Path of images that are not yet assigned to a review
pub fn get_pending_path() -> PathBuf {
    PENDING_PATH.iter().collect()
}

// Path where images of unapproved reviews are stored
pub fn get_unapproved_path() -> PathBuf {
    UNAPPROVED_PATH.iter().collect()
}

// Path where public servable images are stored
pub fn get_original_path() -> PathBuf {
    ORIGINAL_PATH.iter().collect()
}

// Path for image cache
pub fn get_cache_path() -> PathBuf {
    CACHE_PATH.iter().collect()
}

pub fn get_raw_path() -> PathBuf {
    RAW_PATH.iter().collect()
}

use std::path::PathBuf;

use crate::constants::{CACHE_PATH, ORIGINAL_PATH, PENDING_PATH, UNAPPROVED_PATH};

// Path of images that are not yet assigned to a review
pub fn get_pending_path() -> PathBuf {
    return PENDING_PATH.iter().collect();
}

// Path where images of unapproved reviews are stored
pub fn get_unapproved_path() -> PathBuf {
    return UNAPPROVED_PATH.iter().collect();
}

// Path where public servable images are stored
pub fn get_original_path() -> PathBuf {
    return ORIGINAL_PATH.iter().collect();
}

// Path for image cache
pub fn get_cache_path() -> PathBuf {
    return CACHE_PATH.iter().collect();
}

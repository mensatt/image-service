use std::{
    fs::{read_dir, remove_file, DirEntry},
    io, thread,
    time::{Duration, SystemTime},
};

use crate::util::path::{get_pending_path, get_raw_path};

pub fn delete_old_pending_images() {
    loop {
        // Get the current time
        let current_time = SystemTime::now();

        // Define the threshold for file deletion (1 hour ago)
        let threshold = current_time - Duration::from_secs(3600);

        log::info!("Starting deletion of old pending files.");

        // Get iterator to iterate over all entries in PENDING_PATH directory
        match read_dir(get_pending_path()) {
            Err(err) => log::error!("Unable to read pending path: {}", err),
            Ok(iterator) => iterator.for_each(|dir_entry| dir_entry_handler(dir_entry, threshold)),
        }

        log::info!("Finished deletion of old pending files. Going back to sleep...");
        // Sleep for 15 minutes
        thread::sleep(Duration::from_secs(900));
    }
}

/// Takes a `DirEntry` as a Result and deletes it if it is:
/// - a regular file and
/// - not a hidden file and
/// - older than `threshold`
fn dir_entry_handler(dir_entry_res: Result<DirEntry, io::Error>, threshold: SystemTime) {
    let dir_entry = match dir_entry_res {
        Err(err) => {
            log::error!("Error while reading dir entry: {}", err);
            return;
        }
        Ok(dir_entry) => dir_entry,
    };

    // If the entry is a regular file
    if dir_entry.path().is_file() {
        // Ignore hidden files
        let file_name = dir_entry.file_name();
        let file_name_str = match file_name.to_str() {
            None => {
                log::error!("Unable to get file name as string for: '{:?}'", dir_entry);
                return;
            }
            Some(name) => name,
        };

        if file_name_str.starts_with('.') {
            return;
        }

        // Get modified time
        let modified_time = match dir_entry.metadata() {
            Err(err) => {
                log::error!(
                    "Unable to get metadata for '{:?}': {}",
                    dir_entry.path(),
                    err
                );
                return;
            }
            Ok(metadata) => match metadata.modified() {
                Err(err) => {
                    log::error!(
                        "Unable to get modified time for '{:?}': {}",
                        dir_entry.path(),
                        err
                    );
                    return;
                }
                Ok(modified_time) => modified_time,
            },
        };

        // Delete the file if it's older than the threshold
        if modified_time < threshold {
            match remove_file(dir_entry.path()) {
                Err(err) => log::error!("Unable to delete '{:?}': {}", dir_entry.path(), err),
                Ok(_) => {
                    log::info!("Deleted {:?}", dir_entry.path());

                    // Also delete the corresponding raw image with the same UUID
                    delete_corresponding_raw_image(file_name_str);
                }
            }
        }
    }
}

/// Deletes the raw image file with the same filename (UUID) from the raw directory
fn delete_corresponding_raw_image(filename: &str) {
    // Extract the UUID from the filename (before the file extension)
    let uuid = match filename.rsplit_once('.') {
        Some((uuid, _ext)) => uuid,
        None => {
            log::warn!("Unable to extract UUID from filename: {}", filename);
            return;
        }
    };

    // Look for any file in the raw directory with this UUID
    let raw_path = get_raw_path();
    match read_dir(&raw_path) {
        Err(err) => log::error!("Unable to read raw path: {}", err),
        Ok(entries) => {
            for entry_res in entries {
                let entry = match entry_res {
                    Ok(e) => e,
                    Err(err) => {
                        log::error!("Error reading raw directory entry: {}", err);
                        continue;
                    }
                };

                let raw_filename = match entry.file_name().to_str() {
                    Some(name) => name.to_string(),
                    None => continue,
                };

                // Check if the filename starts with the same UUID
                if raw_filename.starts_with(uuid) {
                    match remove_file(entry.path()) {
                        Err(err) => log::error!("Unable to delete raw file '{:?}': {}", entry.path(), err),
                        Ok(_) => log::info!("Deleted corresponding raw image: {:?}", entry.path()),
                    }
                    break; // Stop after finding and deleting the matching file
                }
            }
        }
    }
}

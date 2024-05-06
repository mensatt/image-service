use crate::util::path::get_cache_path;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::fs::{read_dir, remove_file, DirEntry};
use std::io;
use std::path::PathBuf;
use uuid::Uuid;
use IteratorContinuation::Continue;

#[derive(PartialEq)]
pub enum CacheBehavior {
    Normal,
    Skip,
}

pub struct CacheInformation {
    entries: u64,
    total_size: u64,
    height_count: HashMap<i32, u64>,
    width_count: HashMap<i32, u64>,
    quality_count: HashMap<i32, u64>,
}

impl Display for CacheInformation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cache Information:\nTotal Entries: {}\nTotal Size: {} bytes\n",
            self.entries, self.total_size
        )?;
        writeln!(f, "Height counts:")?;
        for (height, count) in &self.height_count {
            writeln!(f, "- {}: {}", height, count)?;
        }

        writeln!(f, "Width counts:")?;
        for (width, count) in &self.width_count {
            writeln!(f, "- {}: {}", width, count)?;
        }

        writeln!(f, "Quality counts:")?;
        for (quality, count) in &self.quality_count {
            writeln!(f, "- {}: {}", quality, count)?;
        }

        Ok(())
    }
}

// Not quite the same as std::ops::ControlFlow
// It requires generic types, which are not needed here
#[allow(dead_code)]
enum IteratorContinuation {
    Continue,
    Break,
}

struct CacheEntry {
    uuid: Uuid,
    height: i32,
    width: i32,
    quality: i32,
}

impl TryFrom<&str> for CacheEntry {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let extension_split = value.split('.').collect::<Vec<_>>();
        if extension_split.len() != 2 {
            return Err("No file extension found".to_string());
        }
        // Now: ["5e90308-d50a-4a45-9718-cca6b7b90d7b-966x2048-80", "webp"]
        let mut dash_split = extension_split[0].rsplitn(3, '-').collect::<Vec<_>>();
        // `rsplitn` iterates from the right, thus we need to reverse the values to get to the
        // intuitive version.
        dash_split.reverse();

        if dash_split.len() != 3 {
            return Err("Unrecognized cache file name".to_string());
        }
        // Now: ["5e90308-d50a-4a45-9718-cca6b7b90d7b", "966x2048", "80"];

        let uuid = Uuid::parse_str(dash_split[0]).map_err(|_| "Invalid UUID")?;
        let dimensions = dash_split[1].split('x').collect::<Vec<_>>();
        if dimensions.len() != 2 {
            return Err("Invalid dimensions".to_string());
        }

        let height = dimensions[0].parse::<i32>().map_err(|_| "Invalid height")?;
        let width = dimensions[1].parse::<i32>().map_err(|_| "Invalid width")?;
        let quality = dash_split[2]
            .parse::<i32>()
            .map_err(|_| "Invalid quality")?;
        Ok(CacheEntry {
            uuid,
            height,
            width,
            quality,
        })
    }
}

// Note: This is not performant. However, as this can only be queried from authorized users,
// it should not be an issue.
pub fn cache_status() -> Result<CacheInformation, io::Error> {
    let mut cache_info = CacheInformation {
        entries: 0,
        total_size: 0,
        height_count: HashMap::new(),
        width_count: HashMap::new(),
        quality_count: HashMap::new(),
    };

    iterate_cache_entries(|dir_entry, name| {
        match CacheEntry::try_from(name) {
            Ok(cache_entry) => {
                cache_info.entries += 1;
                let metadata = dir_entry.metadata()?;
                cache_info.total_size += metadata.len();

                let hc = cache_info
                    .height_count
                    .entry(cache_entry.height)
                    .or_insert(0);
                *hc += 1;

                let wc = cache_info.width_count.entry(cache_entry.width).or_insert(0);
                *wc += 1;

                let dc = cache_info
                    .quality_count
                    .entry(cache_entry.quality)
                    .or_insert(0);
                *dc += 1;
            }
            Err(err) => {
                log::warn!("Could not determine cache entry for file {}: {}", name, err)
            }
        }

        Ok(Continue)
    })?;

    Ok(cache_info)
}

pub fn get_cache_entry(uuid: &str, height: i32, width: i32, quality: i32) -> PathBuf {
    get_cache_path().join(format!("{}-{}x{}-{}.webp", uuid, width, height, quality))
}

pub fn check_cache(uuid: Uuid, height: i32, width: i32, quality: i32) -> bool {
    let cache_entry = get_cache_entry(&uuid.to_string(), height, width, quality);
    cache_entry.exists()
}

pub fn remove_cache_entries(uuid: Uuid) {
    let cache_iter_res = iterate_cache_entries(|dir_entry, name| {
        // Ignore unwanted files
        if !name.starts_with(&uuid.to_string()) {
            return Ok(Continue);
        }

        match remove_file(dir_entry.path()) {
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => (), // Can be ignored
                _ => log::error!("Unable to delete '{:?}': {}", dir_entry.path(), err),
            },
            Ok(_) => {
                log::info!("Deleted '{:?}'", dir_entry.path())
            }
        }
        Ok(Continue)
    });

    match cache_iter_res {
        Ok(_) => {}
        Err(_) => {
            log::error!("Encountered error while removing cache entry {}", uuid);
        }
    }
}

fn iterate_cache_entries<F>(mut f: F) -> Result<(), io::Error>
where
    F: FnMut(DirEntry, &str) -> Result<IteratorContinuation, io::Error>,
{
    match read_dir(get_cache_path()) {
        Err(err) => {
            log::error!("Unable to read cache path: {}", err);
            return Err(err);
        }
        Ok(iterator) => {
            for dir_entry in iterator {
                match dir_entry {
                    Err(err) => {
                        log::error!("Unable to read dir entry: {}", err);
                        return Err(err);
                    }
                    Ok(dir_entry) => {
                        let file_name = dir_entry.file_name();
                        let file_name_str = match file_name.to_str() {
                            None => {
                                log::error!(
                                    "Unable to get file name as string for: '{:?}'",
                                    dir_entry
                                );
                                // TODO: Don't suppress error
                                continue;
                            }
                            Some(name) => name,
                        };

                        let res = f(dir_entry, file_name_str)?;
                        match res {
                            Continue => {}
                            IteratorContinuation::Break => break,
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

use core::fmt;
use std::{
    fs::{read_dir, remove_file, rename},
    io,
    path::{Path, PathBuf},
};

use axum::body::Bytes;
use libvips::{
    bindings::VIPS_MAX_COORD,
    ops::{self, ForeignHeifCompression, HeifsaveOptions},
    VipsImage,
};
use uuid::Uuid;

use crate::util::path::get_raw_path;
use crate::{
    constants::PENDING_QUALITY,
    util::path::{get_cache_path, get_original_path, get_pending_path, get_unapproved_path},
};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, PartialEq)]
pub enum FileType {
    JPEG,
    PNG,
    WEBP,
    HEIF,
    AVIF,
}

#[allow(dead_code)]
pub struct FileIdentification {
    file_type: FileType,
    file_extension: &'static str,
    file_header: &'static [u8],
}

#[derive(Debug)]
pub enum SaveError {
    LibError(libvips::error::Error),
    IOError(std::io::Error),
}

#[derive(PartialEq)]
pub enum CacheBehavior {
    Normal,
    Skip,
}

#[allow(dead_code)]
#[derive(PartialEq)]
pub enum ImageSearchBehaviour {
    All,
    // Valid means, that the image is properly attached to a review, be it unapproved or not
    Valid,
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LibError(err) => err.fmt(f),
            Self::IOError(err) => err.fmt(f),
        }
    }
}

const FILE_MAPPINGS: [FileIdentification; 5] = [
    FileIdentification {
        file_type: FileType::JPEG,
        file_extension: "jpg",
        file_header: &[0xff, 0xd8, 0xff],
    },
    FileIdentification {
        file_type: FileType::PNG,
        file_extension: "png",
        file_header: &[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a],
    },
    FileIdentification {
        file_type: FileType::WEBP,
        file_extension: "webp",
        file_header: &[0x52, 0x49, 0x46, 0x46],
    },
    FileIdentification {
        file_type: FileType::HEIF,
        file_extension: "heic",
        file_header: &[
            0x00, 0x00, 0x00, 0x18, 0x66, 0x74, 0x79, 0x70, 0x68, 0x65, 0x69, 0x63,
        ],
    },
    FileIdentification {
        file_type: FileType::AVIF,
        file_extension: "avif",
        file_header: &[
            0x00, 0x00, 0x00, 0x1c, 0x66, 0x74, 0x79, 0x70, 0x61, 0x76, 0x69, 0x66,
        ],
    },
];

pub fn determine_file_type(image: &Bytes) -> Option<&FileIdentification> {
    FILE_MAPPINGS
        .iter()
        .find(|&mapping| image.starts_with(mapping.file_header))
}

pub fn save_raw(data: &Bytes, uuid: Uuid) -> Result<(), SaveError> {
    let path = get_raw_path().join(format!("{}.raw", uuid));
    let path_str = match path.to_str() {
        Some(str) => str,
        None => {
            return Err(SaveError::IOError(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not determine path string",
            )));
        }
    };

    log::info!("Saving raw image to {}", path_str);
    std::fs::write(path_str, data.as_ref()).map_err(SaveError::IOError)
}

pub fn save_pending(data: &Bytes, uuid: Uuid, angle: f64) -> Result<(), SaveError> {
    let path = get_pending_path().join(format!("{}.avif", uuid));
    let path_str = match path.to_str() {
        Some(str) => str,
        None => {
            return Err(SaveError::IOError(io::Error::new(
                io::ErrorKind::InvalidData,
                "Could not determine path string",
            )));
        }
    };
    log::info!("Saving pending image to {}", path_str);

    let image = match VipsImage::new_from_buffer(data, "") {
        Err(err) => {
            log::error!("Error while reading image from buffer: {}", err);
            return Err(SaveError::LibError(err));
        }
        Ok(img) => img,
    };

    let rotated = match ops::rotate(&image, angle) {
        Err(err) => {
            log::error!("Error while rotating '{}': {}", path_str, err);
            return Err(SaveError::LibError(err));
        }
        Ok(img) => img,
    };

    save_image(&rotated, path_str, PENDING_QUALITY)?;

    Ok(())
}

pub fn save_image(image: &VipsImage, path_str: &str, quality: i32) -> Result<(), SaveError> {
    let heifsave_options = HeifsaveOptions {
        q: quality,
        compression: ForeignHeifCompression::Av1,
        effort: 0,
        ..Default::default()
    };

    match ops::heifsave_with_opts(image, path_str, &heifsave_options) {
        Err(err) => {
            log::error!("Error while saving '{}': {}", path_str, err);
            // TODO: heifsave lib has changed and returns "error" on success
            // See:
            //  - https://github.com/libvips/libvips/issues/3718#issuecomment-1771494570
            //  - https://github.com/libvips/libvips/pull/3724
            //  - https://github.com/olxgroup-oss/libvips-rust-bindings/pull/35
            // return Err(SaveError::LibError(err));
        }
        Ok(_) => log::info!("Saved '{}'", path_str),
    }

    Ok(())
}

pub fn determine_img_path(folder: &str, uuid: Uuid) -> Result<PathBuf, io::Error> {
    let buf = PathBuf::from(folder).join(format!("{}.avif", uuid));
    if buf.exists() {
        return Ok(buf);
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("No image with UUID '{}'", uuid),
    ))
}

pub fn determine_img_dir(
    uuid: Uuid,
    search_behaviour: ImageSearchBehaviour,
) -> Result<PathBuf, io::Error> {
    // Search unapproved
    if determine_img_path(get_unapproved_path().to_str().unwrap(), uuid).is_ok() {
        return Ok(get_unapproved_path());
    }

    // Search original
    if determine_img_path(get_original_path().to_str().unwrap(), uuid).is_ok() {
        return Ok(get_original_path());
    }

    if search_behaviour == ImageSearchBehaviour::Valid {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("No valid image with UUID '{}'", uuid),
        ));
    }

    // Search pending
    if determine_img_path(get_pending_path().to_str().unwrap(), uuid).is_ok() {
        return Ok(get_pending_path());
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("No image with UUID '{}'", uuid),
    ))
}

pub fn manipulate_image(
    path: &str,
    height: Option<i32>,
    width: Option<i32>,
    quality: i32,
    cache_behavior: CacheBehavior,
) -> Result<Vec<u8>, libvips::error::Error> {
    // Use MAX_COORD as "infinity" for unspecified dimension
    // See: https://github.com/libvips/libvips/issues/709#issuecomment-373638244
    let target_w = width.unwrap_or(VIPS_MAX_COORD.try_into().unwrap());
    let target_h = height.unwrap_or(VIPS_MAX_COORD.try_into().unwrap());

    // Only crop when both height and width are set
    // Otherwise (if only one or none are present) the image is simply scaled down.
    let crop = match (height, width) {
        (Some(_), Some(_)) => ops::Interesting::Centre,
        _ => ops::Interesting::None,
    };

    let thumb_opts = ops::ThumbnailOptions {
        height: target_h,
        // See https://github.com/olxgroup-oss/libvips-rust-bindings/issues/42
        import_profile: "sRGB".into(),
        export_profile: "sRGB".into(),
        size: ops::Size::Down,
        crop: crop,
        ..ops::ThumbnailOptions::default()
    };

    let image = match ops::thumbnail_with_opts(path, target_w, &thumb_opts) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(img) => img,
    };

    let webpsave_buffer_options = ops::WebpsaveBufferOptions {
        q: quality,
        ..ops::WebpsaveBufferOptions::default()
    };
    let buffer: Vec<u8> = match ops::webpsave_buffer_with_opts(&image, &webpsave_buffer_options) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(vec) => vec,
    };

    // Write image to cache if desired
    if cache_behavior == CacheBehavior::Normal {
        let cache_entry = get_cache_entry(
            PathBuf::from(path).file_stem().unwrap().to_str().unwrap(),
            height,
            width,
            quality,
        );

        let opts = ops::WebpsaveOptions {
            q: quality,
            ..ops::WebpsaveOptions::default()
        };
        match ops::webpsave_with_opts(&image, cache_entry.to_str().unwrap(), &opts) {
            Err(err) => {
                log::error!("{}", err);
                return Err(err);
            }
            Ok(img) => img,
        };
    }

    Ok(buffer)
}

pub fn get_cache_entry(
    uuid: &str,
    height: Option<i32>,
    width: Option<i32>,
    quality: i32,
) -> PathBuf {
    // We use the string "UNSPEC" to indicate that a requested resolution value was left
    // unspecified in the query
    let width_str = width.map_or("UNSPEC".to_string(), |w| w.to_string());
    let height_str = height.map_or("UNSPEC".to_string(), |w| w.to_string());
    get_cache_path().join(format!(
        "{}-{}x{}-{}.webp",
        uuid, width_str, height_str, quality
    ))
}

pub fn check_cache(uuid: Uuid, height: Option<i32>, width: Option<i32>, quality: i32) -> bool {
    get_cache_entry(&uuid.to_string(), height, width, quality).exists()
}

pub fn remove_cache_entries(uuid: Uuid) {
    match read_dir(get_cache_path()) {
        Err(err) => log::error!("Unable to read pending path: {}", err),
        Ok(iterator) => {
            iterator.for_each(|dir_entry_res| {
                match dir_entry_res {
                    Err(err) => log::error!("Error while reading dir entry: {}", err),
                    Ok(dir_entry) => {
                        if dir_entry.path().is_file() {
                            let file_name = dir_entry.file_name();
                            let file_name_str = match file_name.to_str() {
                                None => {
                                    log::error!(
                                        "Unable to get file name as string for: '{:?}'",
                                        dir_entry
                                    );
                                    return;
                                }
                                Some(name) => name,
                            };

                            // Ignore unwanted files
                            if !file_name_str.starts_with(&uuid.to_string()) {
                                return;
                            }

                            match remove_file(dir_entry.path()) {
                                Err(err) => match err.kind() {
                                    io::ErrorKind::NotFound => (), // Can be ignored
                                    _ => log::error!(
                                        "Unable to delete '{:?}': {}",
                                        dir_entry.path(),
                                        err
                                    ),
                                },
                                Ok(_) => log::info!("Deleted '{:?}'", dir_entry.path()),
                            }
                        }
                    }
                }
            })
        }
    }
}

pub fn move_image(from: &Path, to: &Path, uuid: Uuid) -> Result<(), io::Error> {
    // Make sure image with given uuid does exist at source path
    let source_path = match determine_img_path(from.to_str().unwrap(), uuid) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(str) => str,
    };

    let target_path = to.join(source_path.file_name().unwrap().to_str().unwrap());

    match rename(&source_path, &target_path) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(_) => log::info!("Moved '{:?}' to '{:?}'", source_path, target_path),
    };

    Ok(())
}

/// Deletes an image with the specified `uuid` from `from`  
/// Returns an io::Error if an error (apart from file not found - which is the expected state)
/// was encountered.
pub fn delete_image(from: &Path, uuid: Uuid) -> Result<(), io::Error> {
    match determine_img_path(from.to_str().unwrap(), uuid) {
        Err(err) => match err.kind() {
            // If the file is not found, everything is as expected
            io::ErrorKind::NotFound => (),
            // Some other error occurred, we should return it
            _ => {
                log::error!("Error while getting path for '{}': {}", uuid, err);
                return Err(err);
            }
        },
        Ok(path) => {
            if let Err(err) = std::fs::remove_file(&path) {
                match err.kind() {
                    // If the file is not found, everything is as expected (although this should have returned above)
                    io::ErrorKind::NotFound => (),
                    // Some other error occurred, we should return it
                    _ => {
                        log::error!("Error while removing '{:?}': {}", path, err);
                        return Err(err);
                    }
                }
            }
        }
    };
    Ok(())
}

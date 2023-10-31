use core::fmt;
use std::{io, path::PathBuf};

use axum::body::Bytes;
use libvips::{
    ops::{self, ForeignHeifCompression, HeifsaveOptions},
    VipsImage,
};
use uuid::Uuid;

use crate::{
    constants::PENDING_QUAL,
    path_utils::{get_cache_path, get_pending_path},
};

#[derive(Debug, PartialEq)]
pub enum FileType {
    JPEG,
    PNG,
    WEBP,
    HEIF,
    AVIF,
}

pub struct FileIdentification {
    file_type: FileType,
    file_extension: &'static str,
    file_header: &'static [u8],
}

pub enum SaveError {
    LibError(libvips::error::Error),
    IOError(std::io::Error),
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

pub fn save_pending(data: &Bytes, uuid: Uuid, angle: f64) -> Result<(), SaveError> {
    let path = get_pending_path().join(format!("{}.avif", uuid.to_string()));
    let path_str = match path.to_str() {
        Some(str) => str,
        None => {
            return Err(SaveError::IOError(io::Error::new(
                io::ErrorKind::InvalidData,
                "error",
            )))
        }
    };
    log::info!("{}", path_str);

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

    let heifsave_options = HeifsaveOptions {
        q: PENDING_QUAL,
        compression: ForeignHeifCompression::Av1,
        ..Default::default()
    };

    match ops::heifsave_with_opts(&rotated, path_str, &heifsave_options) {
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
    };

    return Ok(());
}

pub fn determine_img_dim(path: &str) -> Result<(i32, i32), libvips::error::Error> {
    match VipsImage::new_from_file(path) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(img) => return Ok((img.get_width(), img.get_height())),
    };
}

pub fn determine_img_path(folder: &str, uuid: Uuid) -> Result<PathBuf, io::Error> {
    let buf = PathBuf::from(folder).join(format!("{}.avif", uuid.to_string()));
    if buf.exists() {
        return Ok(buf);
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("No image with UUID '{}'", uuid),
    ))
}

pub fn manipulate_image(
    path: &str,
    height: i32,
    width: i32,
    quality: i32,
) -> Result<Vec<u8>, libvips::error::Error> {
    let thumb_opts = ops::ThumbnailOptions {
        height: height,
        // See https://github.com/olxgroup-oss/libvips-rust-bindings/issues/42
        import_profile: "sRGB".into(),
        export_profile: "sRGB".into(),
        crop: ops::Interesting::Centre,
        ..ops::ThumbnailOptions::default()
    };
    let image = match ops::thumbnail_with_opts(path, width, &thumb_opts) {
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

    // Also write image to cache
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

    return Ok(buffer);
}

pub fn get_cache_entry(uuid: &str, height: i32, width: i32, quality: i32) -> PathBuf {
    return get_cache_path().join(format!("{}-{}x{}-{}.webp", uuid, width, height, quality));
}

pub fn check_cache(uuid: Uuid, height: i32, width: i32, quality: i32) -> bool {
    let cache_entry = get_cache_entry(&uuid.to_string(), height, width, quality);
    return cache_entry.exists();
}

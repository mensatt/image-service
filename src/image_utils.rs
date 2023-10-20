use core::fmt;
use std::{fs, io};

use axum::body::Bytes;
use libvips::{ops, VipsImage};
use uuid::Uuid;

#[derive(Debug, PartialEq)]
pub enum FileType {
    JPEG,
    PNG,
    WEBP,
    HEIF,
}

pub struct FileIdentification {
    file_type: FileType,
    file_extension: &'static str,
    file_header: &'static [u8],
}

const FILE_MAPPINGS: [FileIdentification; 4] = [
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
];

/*

func isImageValid(image []byte) bool {
    for _, magicNumber := range magicNumbers {
        if isMagicNumberValid(image, magicNumber) {
            return true
        }
    }
    return false
}

func isMagicNumberValid(image []byte, magicNumber []byte) bool {
    if len(image) < len(magicNumber) {
        return false
    }

    for i := range magicNumber {
        if image[i] != magicNumber[i] {
            return false
        }
    }
    return true
}

 */
pub fn determine_file_type(image: &[u8]) -> Option<&FileIdentification> {
    FILE_MAPPINGS
        .iter()
        .find(|&mapping| image.starts_with(mapping.file_header))
}

pub enum SaveError {
    IOError(io::Error),
    LibError(libvips::error::Error),
}
impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SaveError::IOError(err) => err.fmt(f),
            SaveError::LibError(err) => err.fmt(f),
        }
    }
}

pub fn save_unmodified(
    data: Bytes,
    uuid: Uuid,
    extension: &str,
    angle: f64,
) -> Result<(), SaveError> {
    // TODO: Path should likely be constructed with some sort of OS util
    let new_name = "uploads/".to_owned() + &uuid.to_string() + "." + extension;
    let webp_name = "uploads/".to_owned() + &uuid.to_string() + ".webp";

    match fs::write(&new_name, &data) {
        Err(err) => {
            log::error!("Error while writing '{}': {}", new_name, err);
            return Err(SaveError::IOError(err));
        }
        Ok(_) => (),
    };

    log::info!("Saved '{}'", new_name);

    let image = match VipsImage::new_from_buffer(&data, "") {
        Err(err) => {
            log::error!("Error while reading image from buffer: {}", err);
            return Err(SaveError::LibError(err));
        }
        Ok(img) => img,
    };

    let rotated = match ops::rotate(&image, angle) {
        Err(err) => {
            log::error!("Error while rotating '{}': {}", new_name, err);
            return Err(SaveError::LibError(err));
        }
        Ok(img) => img,
    };

    let opts = ops::WebpsaveOptions {
        // TODO: Do not convert images to webp on upload, save them as is
        // WEBP-lossless only inflates image (tested 1.3 MB JPEG => ~3 MB WEBP)
        // lossless: true,
        ..ops::WebpsaveOptions::default()
    };
    match ops::webpsave_with_opts(&rotated, &webp_name, &opts) {
        Ok(_) => log::info!("Saved '{}'", webp_name),
        Err(err) => {
            log::error!("Error while writing '{}': {}", webp_name, err);
            return Err(SaveError::LibError(err));
        }
    }

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

pub fn manipulate_image(
    path: &str,
    height: i32,
    width: i32,
    quality: i32,
) -> Result<(), libvips::error::Error> {
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

    // TODO: Figure out if there is a better way of passing the image.
    // Ideas (tried but failed, worth investigating further): Returning image or writing to buffer
    let opts = ops::WebpsaveOptions {
        q: quality,
        ..ops::WebpsaveOptions::default()
    };
    match ops::webpsave_with_opts(&image, "temp.webp", &opts) {
        Err(err) => {
            log::error!("{}", err);
            return Err(err);
        }
        Ok(img) => img,
    };
    return Ok(());
}

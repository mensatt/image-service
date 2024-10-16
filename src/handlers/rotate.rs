use crate::util::image::remove_cache_entries;
use crate::{
    util::{
        auth::check_auth_header,
        image::{determine_img_dir, determine_img_path, save_image, ImageSearchBehaviour},
    },
    ServerState,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use libvips::{ops, VipsImage};
use serde::Deserialize;
use std::fs::rename;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct RotateQuery {
    id: Uuid,
    angle: i64,
}

pub async fn rotate_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    query: Query<RotateQuery>,
) -> Result<String, (StatusCode, String)> {
    check_auth_header(authorization, &server_state.api_key_hashes)?;

    if query.angle <= 0 || query.angle >= 360 || query.angle % 90 != 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Angle must be one of {90, 180, 270}!".to_owned(),
        ));
    }

    let image_directory = match determine_img_dir(query.id, ImageSearchBehaviour::Valid) {
        Ok(image_directory) => image_directory,
        Err(_) => return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned())),
    };

    let image_directory_string = image_directory.to_string_lossy().to_string();

    let image_path = match determine_img_path(image_directory_string.as_str(), query.id) {
        Err(err) => {
            log::warn!(
            "Image not found where the path was previously determined. Id: {:?}, Directory: {:?}, Error: {:?}",
            query.id,
            image_directory_string.as_str(),
            err
        );
            return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()));
        }
        Ok(image_path) => image_path,
    };

    let image_path_string = image_path.to_string_lossy().to_string();

    let image = match VipsImage::new_from_file(image_path_string.as_str()) {
        Ok(image) => image,
        Err(err) => {
            log::error!(
                "Error while opening image. Id: {:?}, Error: {:?}, Path: {:?}",
                query.id,
                err,
                image_path
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while opening image!".to_owned(),
            ));
        }
    };

    let rotated = match ops::rotate(&image, query.angle as f64) {
        Ok(rotated) => rotated,
        Err(err) => {
            log::error!(
                "Error while rotating image. Id: {:?}, Error: {:?}",
                query.id,
                err
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while rotating image!".to_owned(),
            ));
        }
    };

    let rotated_image_path = format!(
        "{}-rotation{}.avif",
        image_path_string.strip_suffix(".avif").unwrap(),
        query.angle
    );

    match save_image(&rotated, rotated_image_path.as_str()) {
        Ok(_) => (),
        Err(err) => {
            log::error!(
                "Error while saving image. Id: {:?}, Error: {:?}",
                query.id,
                err
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while saving image!".to_owned(),
            ));
        }
    }

    match rename(rotated_image_path, image_path) {
        Ok(_) => (),
        Err(err) => {
            log::error!(
                "Error while renaming image. Id: {:?}, Error: {:?}",
                query.id,
                err
            );
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while renaming image!".to_owned(),
            ));
        }
    }

    remove_cache_entries(query.id);

    Ok(query.id.to_string())
}

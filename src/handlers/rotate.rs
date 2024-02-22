use crate::util::image::{move_image, remove_cache_entries};
use crate::{
    util::{
        auth::check_api_key,
        image::{determine_img_dir, determine_img_path, save_image, ImageSearchBehaviour},
    },
    ServerState,
};
use axum::extract::{Query, State};
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::StatusCode;
use axum::TypedHeader;
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
    check_api_key(authorization, &server_state.api_key_hash)?;

    if query.angle <= 0 || query.angle >= 360 || query.angle % 90 != 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "Angle must be one of {90, 180, 270}!".to_owned(),
        ));
    }

    let image_directory = determine_img_dir(query.id, ImageSearchBehaviour::Valid);
    if image_directory.is_err() {
        return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()));
    }

    let image_directory = image_directory.unwrap();
    let image_directory = image_directory.to_str().unwrap();

    let image_path = determine_img_path(image_directory, query.id);
    if image_path.is_err() {
        log::warn!(
            "Image not found where the path was previously determined. Id: {:?}, Directory: {:?}, Error: {:?}",
            query.id,
            image_directory,
            image_path.err()
        );
        return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()));
    }

    let image_path = image_path.unwrap();
    let image_path = image_path.to_str().unwrap();

    let image = match VipsImage::new_from_file(image_path) {
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

    let rotated = match ops::rotate(&image, query.angle) {
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
        image_path.strip_suffix(".avif").unwrap(),
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

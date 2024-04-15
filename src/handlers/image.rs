use crate::{
    util::{
        auth::{check_auth, check_auth_header},
        image::{
            check_cache, delete_image, determine_img_dim, determine_img_path, get_cache_entry,
            manipulate_image, remove_cache_entries, CacheBehavior,
        },
        path::{get_original_path, get_pending_path, get_unapproved_path},
    },
    ServerState,
};

use axum::{
    extract::{Path, Query, State},
    headers::{authorization::Bearer, Authorization},
    http::{header, StatusCode},
    response::IntoResponse,
    TypedHeader,
};
use serde::Deserialize;
use std::fs::read;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct ImageQuery {
    width: Option<i32>,
    height: Option<i32>,
    quality: Option<i32>,
    auth: Option<String>,
}

// This handler serves images with the given id from the filesystem
// It accepts optional query parameters for width, height and quality
// It also accepts an optional Authorization header and - if it's valid - serves unapproved images
// Images are resized, and compressed using vips
pub async fn image_handler(
    State(server_state): State<ServerState>,
    authorization_header_opt: Option<TypedHeader<Authorization<Bearer>>>,
    Path(id): Path<Uuid>,
    query: Query<ImageQuery>,
) -> impl IntoResponse {
    // Check ID
    if id.is_nil() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ID!".to_owned()));
    }

    // Return image if it exists in original path
    match determine_img_path(get_original_path().to_str().unwrap(), id) {
        Err(_) => (),
        Ok(path) => {
            return image_handler_helper(
                id,
                path.to_str().unwrap(),
                query.0,
                CacheBehavior::Normal,
            );
        }
    };

    let not_found_resp = Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()));
    return match check_auth(
        query.auth.as_ref(),
        authorization_header_opt,
        &server_state.api_key_hashes,
    ) {
        Err(_) => not_found_resp,
        Ok(()) => match determine_img_path(get_unapproved_path().to_str().unwrap(), id) {
            Err(_) => not_found_resp, // Return 404 if image was also not found in unapproved path
            Ok(path) => {
                // Skip cache for unapproved images to avoid leaking them via cache
                image_handler_helper(id, path.to_str().unwrap(), query.0, CacheBehavior::Skip)
            }
        },
    };
}

type Headers = [(header::HeaderName, String); 2];
type Body = Vec<u8>;

/// Takes a uuid, path,an image query and a skip_cache flag and returns the image manipulated by the arguments of image query
/// If a error occurs, an appropriate HTTP status code and message is returned.
fn image_handler_helper(
    uuid: Uuid,
    path: &str,
    image_query: ImageQuery,
    cache_behavior: CacheBehavior,
) -> Result<(Headers, Body), (StatusCode, String)> {
    // Get image dimensions; used as fallback in case height and/or width missing in image_query
    let mut img_dim = match determine_img_dim(path) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while getting image dimensions".to_owned(),
            ));
        }
        Ok(img_dim) => img_dim,
    };

    if img_dim.1 > img_dim.0 {
        // Suspected image rotation
        img_dim = (img_dim.1, img_dim.0)
    }

    // Get arguments for manipulate image
    let width = image_query.width.unwrap_or(img_dim.0);
    let height = image_query.height.unwrap_or(img_dim.1);
    let quality = image_query.quality.unwrap_or(80);

    // Construct HTTP Header
    let headers = [
        (header::CONTENT_TYPE, "image/webp".to_owned()),
        (
            header::CONTENT_DISPOSITION,
            format!("inline; filename={:?}.webp", uuid),
        ),
    ];

    // Construct HTTP Body
    // If cache is desired and requested image is already cached, the cached version is returned
    let body = match cache_behavior {
        CacheBehavior::Normal if check_cache(uuid, height, width, quality) => {
            read(get_cache_entry(&uuid.to_string(), height, width, quality)).unwrap()
        }
        _ => match manipulate_image(path, height, width, quality, cache_behavior) {
            Err(err) => {
                log::error!("{}", err);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error while processing image!".to_owned(),
                ));
            }
            Ok(buf) => buf,
        },
    };

    Ok((headers, body))
}

pub async fn image_delete_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    Path(uuid): Path<Uuid>,
) -> Result<String, (StatusCode, String)> {
    check_auth_header(authorization, &server_state.api_key_hashes)?;

    // Check ID
    if uuid.is_nil() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ID!".to_owned()));
    }

    // To avoid code duplication below
    let internal_server_error = (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error while deleting image!".to_owned(),
    );

    // Make sure image is deleted from pending, unapproved and original paths
    delete_image(&get_pending_path(), uuid)
        .map_err(|_| -> (StatusCode, String) { internal_server_error.clone() })?;
    delete_image(&get_unapproved_path(), uuid)
        .map_err(|_| -> (StatusCode, String) { internal_server_error.clone() })?;
    delete_image(&get_original_path(), uuid)
        .map_err(|_| -> (StatusCode, String) { internal_server_error.clone() })?;
    remove_cache_entries(uuid);

    Ok(uuid.to_string())
}

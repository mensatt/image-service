use std::fs::read;

use axum::{
    extract::{Path, Query, State},
    headers::{authorization::Bearer, Authorization},
    http::{header, StatusCode},
    response::IntoResponse,
    TypedHeader,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    auth_utils::check_api_key,
    image_utils::{
        check_cache, determine_img_dim, determine_img_path, get_cache_entry, manipulate_image,
        CacheBehavior,
    },
    path_utils::{get_original_path, get_unapproved_path},
    ServerState,
};

#[derive(Deserialize)]
pub struct ImageQuery {
    width: Option<i32>,
    height: Option<i32>,
    quality: Option<i32>,
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
            return image_handler_helper(id, path.to_str().unwrap(), query.0, CacheBehavior::Normal)
        }
    };

    let not_found_resp = Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()));
    match authorization_header_opt {
        // Return 404 if no header was present
        // Note: Image was not found in original path, otherwise we would have returned above
        None => return not_found_resp,
        Some(TypedHeader(authorization)) => {
            match check_api_key(authorization, &server_state.api_key_hash) {
                Err(_) => return not_found_resp, // Return 404 if API Key was invalid
                Ok(()) => match determine_img_path(get_unapproved_path().to_str().unwrap(), id) {
                    Err(_) => return not_found_resp, // Return 404 if image was also not found in unapproved path
                    Ok(path) => {
                        // Skip cache for unapproved images to avoid leaking them via cache
                        return image_handler_helper(
                            id,
                            path.to_str().unwrap(),
                            query.0,
                            CacheBehavior::Skip,
                        );
                    }
                },
            }
        }
    }
}

/// Takes a uuid, path,an image query and a skip_cache flag and returns the image manipulated by the arguments of image query  
/// If a error occurs, an appropriate HTTP status code and message is returned.
fn image_handler_helper(
    uuid: Uuid,
    path: &str,
    image_query: ImageQuery,
    cache_behavior: CacheBehavior,
) -> Result<([(header::HeaderName, String); 2], Vec<u8>), (StatusCode, String)> {
    // Get image dimensions; used as fallback in case height and/or width missing in image_query
    let img_dim = match determine_img_dim(path) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while getting image dimensions".to_owned(),
            ));
        }
        Ok(img_dim) => img_dim,
    };

    // Get arguments for manipulate image
    let width = match image_query.width {
        Some(width) => width,
        None => img_dim.0,
    };
    let height = match image_query.height {
        Some(height) => height,
        None => img_dim.1,
    };
    let quality = match image_query.quality {
        Some(quality) => quality,
        None => 100,
    };

    // Construct HTTP Header
    let headers = [
        (header::CONTENT_TYPE, "image/webp".to_owned()),
        (
            header::CONTENT_DISPOSITION,
            format!("attachment; filename={:?}.webp", uuid),
        ),
    ];

    // Construct HTTP Body
    // If requested image is found in cache, the cached version is returned
    let body = match check_cache(uuid, height, width, quality) {
        true => read(get_cache_entry(&uuid.to_string(), height, width, quality)).unwrap(),
        false => match manipulate_image(path, height, width, quality, cache_behavior) {
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

    return Ok((headers, body));
}
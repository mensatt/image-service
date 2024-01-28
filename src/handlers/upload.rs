use axum::{
    extract::{Multipart, Query},
    http::StatusCode,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    constants::CONTENT_LENGTH_LIMIT,
    util::image::{determine_file_type, save_pending},
};

#[derive(Deserialize)]
pub struct UploadQuery {
    angle: Option<f64>,
}

/// This function handles image uploads. An image is expected to be part of a multipart stream.\
/// Only one image (the first field in the stream) is processed.
///
/// Arguments:
///  - query: HTTP Query parameters
///     - angle: To rotate image before saving. Default 0.
///  - multipart: Multipart stream
pub async fn upload_handler(
    query: Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<String, (StatusCode, String)> {
    // Get first Multipart field
    let field = match multipart.next_field().await {
        Err(err) => {
            log::error!("{}", err.body_text());
            return Err((StatusCode::BAD_REQUEST, "No fields provided!".to_owned()));
        }
        Ok(next_field) => match next_field {
            None => return Err((StatusCode::BAD_REQUEST, "No fields provided!".to_owned())),
            Some(field) => field,
        },
    };

    // Get name and data from field
    let name = field.name().unwrap().to_string();
    let data = match field.bytes().await {
        Err(err) => {
            log::error!("{}", err.body_text());
            match err.status() {
                StatusCode::PAYLOAD_TOO_LARGE => {
                    return Err((
                        StatusCode::PAYLOAD_TOO_LARGE,
                        format!(
                            "Content length limit exceeded!. Max allowed file size is {}B",
                            CONTENT_LENGTH_LIMIT
                        ),
                    ));
                }
                _ => return Err((err.status(), "An error occurred your request".to_owned())),
            }
        }
        Ok(data) => data,
    };
    log::info!("Received '{}' with size {}B", name, data.len());

    if data.len() == 0 {
        return Err((StatusCode::BAD_REQUEST, "Empty file provided!".to_owned()));
    }

    let uuid = Uuid::new_v4();
    let angle = match query.angle {
        Some(x) => x,
        None => 0.0,
    };

    match determine_file_type(&data) {
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                "File type could not be determined or your file type is not supported!".to_owned(),
            ))
        }
        Some(_) => (),
    };

    match save_pending(&data, uuid, angle) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "An internal error has occurred!".to_owned(),
            ));
        }
        Ok(_) => (),
    };

    return Ok(uuid.to_string());
}

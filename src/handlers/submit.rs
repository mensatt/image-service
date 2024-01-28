use crate::util::{
    image::move_image,
    path::{get_pending_path, get_unapproved_path},
};

use axum::{extract::Path, http::StatusCode, response::IntoResponse};
use uuid::Uuid;

pub async fn submit_handler(Path(uuid): Path<Uuid>) -> impl IntoResponse {
    // Check ID
    if uuid.is_nil() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ID!".to_owned()));
    }

    match move_image(
        get_pending_path().as_path(),
        get_unapproved_path().as_path(),
        uuid,
    ) {
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()))
            }
            _ => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error while submitting image!".to_owned(),
                ))
            }
        },
        Ok(_) => return Ok(uuid.to_string()),
    };
}

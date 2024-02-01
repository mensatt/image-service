use crate::{
    util::{
        auth::check_api_key,
        image::move_image,
        path::{get_pending_path, get_unapproved_path},
    },
    ServerState,
};

use axum::{
    extract::{Path, State},
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    response::IntoResponse,
    TypedHeader,
};
use uuid::Uuid;

pub async fn submit_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    Path(uuid): Path<Uuid>,
) -> impl IntoResponse {
    check_api_key(authorization, &server_state.api_key_hash)?;
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

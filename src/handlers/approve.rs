use crate::{
    util::{
        auth::check_api_key,
        image::move_image,
        path::{get_original_path, get_unapproved_path},
    },
    ServerState,
};

use axum::{
    extract::{Path, State},
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    TypedHeader,
};
use uuid::Uuid;

// TODO: Add cron pruning
pub async fn approve_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    Path(uuid): Path<Uuid>,
) -> Result<String, (StatusCode, String)> {
    check_api_key(authorization, &server_state.api_key_hash)?;

    // Check ID
    if uuid.is_nil() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ID!".to_owned()));
    }

    return match move_image(
        get_unapproved_path().as_path(),
        get_original_path().as_path(),
        uuid,
    ) {
        Err(err) => match err.kind() {
            std::io::ErrorKind::NotFound => {
                Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()))
            }
            _ => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while approving image!".to_owned(),
            )),
        },
        Ok(_) => Ok(uuid.to_string()),
    };
}

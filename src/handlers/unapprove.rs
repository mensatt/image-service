use crate::{
    util::{
        auth::check_auth_header,
        image::{move_image, remove_cache_entries},
        path::{get_original_path, get_unapproved_path},
    },
    ServerState,
};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use uuid::Uuid;

pub async fn unapprove_handler(
    State(server_state): State<ServerState>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    Path(uuid): Path<Uuid>,
) -> impl IntoResponse {
    check_auth_header(authorization, &server_state.api_key_hashes)?;

    // Check ID
    if uuid.is_nil() {
        return Err((StatusCode::BAD_REQUEST, "Invalid ID!".to_owned()));
    }

    if let Err(err) = move_image(
        get_original_path().as_path(),
        get_unapproved_path().as_path(),
        uuid,
    ) {
        return match err.kind() {
            std::io::ErrorKind::NotFound => {
                Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()))
            }
            _ => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while unapproving image!".to_owned(),
            )),
        };
    };

    remove_cache_entries(uuid);

    Ok(uuid.to_string())
}

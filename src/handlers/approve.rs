use std::fs::rename;

use axum::{
    extract::{Path, State},
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
    TypedHeader,
};
use uuid::Uuid;

use crate::{
    util::{
        auth::check_api_key,
        image::determine_img_path,
        path::{get_original_path, get_pending_path},
    },
    ServerState,
};

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

    let source_path = match determine_img_path(get_pending_path().to_str().unwrap(), uuid) {
        Err(err) => {
            log::error!("{}", err);
            return Err((StatusCode::NOT_FOUND, "Image not found!".to_owned()));
        }
        Ok(str) => str,
    };

    let target_path = get_original_path().join(source_path.file_name().unwrap().to_str().unwrap());

    match rename(&source_path, &target_path) {
        Err(err) => {
            log::error!("{}", err);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Error while approving image!".to_owned(),
            ));
        }
        Ok(_) => log::info!("Moved '{:?}' to '{:?}'", source_path, target_path),
    };

    Ok(uuid.to_string())
}

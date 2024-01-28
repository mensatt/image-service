use argon2::{password_hash::PasswordHashString, Argon2, PasswordVerifier};
use axum::{
    headers::{authorization::Bearer, Authorization},
    http::StatusCode,
};

/// Checks if user is authorized by checking if the given Bearer Token matches the given hash  
/// Returns 401 (UNAUTHORIZED) with appropriate message if they do not match
pub fn check_api_key(
    authorization: Authorization<Bearer>,
    hash: &PasswordHashString,
) -> Result<(), (StatusCode, String)> {
    return match (Argon2::default())
        .verify_password(authorization.token().as_bytes(), &hash.password_hash())
    {
        Err(err) => {
            // TODO: Might want to distinguish between invalid password (4xx) and internal errors (e.g. cryptographic ones)
            log::error!("Error during authentication: {}", err);
            return Err((StatusCode::UNAUTHORIZED, "Invalid token!".to_string()));
        }
        Ok(_) => Ok(()),
    };
}

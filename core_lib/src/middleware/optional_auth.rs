use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};

use crate::{
    error::AppError,
    middleware::auth::AuthUser,
    AppState,
};

pub struct OptionalAuthUser(pub Option<AuthUser>);

#[async_trait]
impl FromRequestParts<AppState> for OptionalAuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = parts.extensions.get::<AuthUser>().cloned();
        Ok(OptionalAuthUser(auth_user))
    }
}
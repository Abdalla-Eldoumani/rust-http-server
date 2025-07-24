//! Middleware integration utilities for applying middleware to specific routes

use crate::config::AppConfig;
use crate::middleware::{
    auth::{jwt_auth_middleware, require_admin, require_role},
    cache::cache_middleware,
};
use crate::auth::models::UserRole;
use crate::AppState;
use axum::{
    middleware as axum_middleware,
    routing::{MethodRouter, Router},
};

pub fn with_auth(router: Router<AppState>) -> Router<AppState> {
    router.layer(axum_middleware::from_fn_with_state(
        AppState::default(),
        jwt_auth_middleware,
    ))
}

pub fn with_admin_auth(method_router: MethodRouter<AppState>) -> MethodRouter<AppState> {
    method_router.layer(axum_middleware::from_fn(require_admin))
}

pub fn with_role_auth(method_router: MethodRouter<AppState>, role: UserRole) -> MethodRouter<AppState> {
    method_router.layer(axum_middleware::from_fn(require_role(role)))
}

pub fn with_cache(method_router: MethodRouter<AppState>) -> MethodRouter<AppState> {
    method_router.layer(axum_middleware::from_fn_with_state(
        AppState::default(),
        cache_middleware,
    ))
}

pub fn with_validation(method_router: MethodRouter<AppState>) -> MethodRouter<AppState> {
    method_router.layer(axum_middleware::from_fn(crate::validation::middleware::validation_middleware))
}

pub fn protected_routes(_config: &AppConfig) -> Router<AppState> {
    Router::new()
        .layer(axum_middleware::from_fn_with_state(
            AppState::default(),
            jwt_auth_middleware,
        ))
}

pub fn admin_routes(_config: &AppConfig) -> Router<AppState> {
    Router::new()
        .layer(axum_middleware::from_fn(require_admin))
        .layer(axum_middleware::from_fn_with_state(
            AppState::default(),
            jwt_auth_middleware,
        ))
}

pub fn apply_middleware_stack(
    router: Router<AppState>,
    state: AppState,
    config: &AppConfig,
) -> Router<AppState> {
    let mut router = router;

    router = router.layer(crate::middleware::cors::cors_layer_from_config(&config.cors));

    router = router.layer(axum_middleware::from_fn_with_state(
        state.clone(),
        crate::middleware::auth::optional_jwt_auth_middleware,
    ));

    if config.rate_limit.enable {
        let rate_limiter = crate::middleware::rate_limit::RateLimiter::new(config.rate_limit.clone());
        router = router.layer(axum_middleware::from_fn_with_state(
            rate_limiter,
            crate::middleware::rate_limit::rate_limit_middleware,
        ));
    }

    router = router.layer(axum_middleware::from_fn_with_state(
        state.clone(),
        cache_middleware,
    ));

    router = router.layer(axum_middleware::from_fn(crate::validation::middleware::validation_middleware));

    router = router.layer(axum_middleware::from_fn(
        crate::middleware::logging::log_request_with_config(config.logging.clone())
    ));

    router
}

pub struct MiddlewareBuilder {
    config: AppConfig,
    state: AppState,
}

impl MiddlewareBuilder {
    pub fn new(config: AppConfig, state: AppState) -> Self {
        Self { config, state }
    }

    pub fn with_auth(self) -> Self {
        self
    }

    pub fn with_admin_only(self) -> Self {
        self
    }

    pub fn with_caching(self) -> Self {
        self
    }

    pub fn with_validation(self) -> Self {
        self
    }

    pub fn build(self, router: Router<AppState>) -> Router<AppState> {
        apply_middleware_stack(router, self.state, &self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{routing::get, Router};

    async fn test_handler() -> &'static str {
        "test"
    }

    #[test]
    fn test_middleware_builder() {
        let config = AppConfig::default();
        let state = AppState::default();
        
        let router = Router::new()
            .route("/test", get(test_handler));
        
        let builder = MiddlewareBuilder::new(config, state);
        let _enhanced_router = builder.build(router);
    }

    #[test]
    fn test_protected_routes() {
        let config = AppConfig::default();
        let _router = protected_routes(&config);
    }

    #[test]
    fn test_admin_routes() {
        let config = AppConfig::default();
        let _router = admin_routes(&config);
    }
}
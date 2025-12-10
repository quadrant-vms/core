use std::sync::Arc;

use crate::service::AuthService;

#[derive(Clone)]
pub struct AuthState {
    service: Arc<AuthService>,
}

impl AuthState {
    pub fn new(service: Arc<AuthService>) -> Self {
        Self { service }
    }

    pub fn service(&self) -> &AuthService {
        &self.service
    }
}

use utoipa::Modify;
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};

pub(crate) struct ApiSecurity;

impl Modify for ApiSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi
            .components
            .get_or_insert_with(utoipa::openapi::Components::new);
        components.add_security_scheme(
            "tanren_session",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("tanren_session"))),
        );
    }
}

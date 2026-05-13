#[path = "service.api.rs"]
mod api;
#[path = "service.i18n.rs"]
mod i18n;

pub use api::ApiService;
pub use i18n::I18nService;

#[allow(unused_imports)]
pub use api::ApiError;

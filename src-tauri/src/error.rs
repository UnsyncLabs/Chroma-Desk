//! Errors returned to the frontend as `{ code, detail }`; the UI translates `code`.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    /// Technical detail (SDK message, path...), shown verbatim next to the translated text.
    pub detail: String,
}

impl ApiError {
    pub fn new(code: &'static str, detail: impl ToString) -> Self {
        Self { code, detail: detail.to_string() }
    }
    /// Error reported by the OpenRGB SDK or the OS.
    pub fn sdk(e: impl ToString) -> Self {
        Self::new("sdk", e)
    }
    pub fn io(e: impl ToString) -> Self {
        Self::new("io", e)
    }
    pub fn no_connection() -> Self {
        Self::new("no_connection", "")
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}

pub type Res<T> = Result<T, ApiError>;

use thiserror::Error;
use wasm_bindgen::JsValue;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("overpass request failed: HTTP {0}")]
    Http(u16),
}

impl From<Error> for JsValue {
    fn from(e: Error) -> Self {
        JsValue::from_str(&e.to_string())
    }
}

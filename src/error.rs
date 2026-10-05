use thiserror::Error;
use wasm_bindgen::JsValue;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("network error: {0}")]
    Network(#[from] gloo_net::Error),
    #[error("overpass request failed: HTTP {0}")]
    Http(u16),
    #[error("map contains no geometry")]
    EmptyMap,
    #[error("could not create surface: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error("no suitable GPU adapter: {0}")]
    RequestAdapter(#[from] wgpu::RequestAdapterError),
    #[error("surface is not supported by the adapter")]
    UnsupportedSurface,
    #[error("could not acquire device: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("could not acquire frame: {0}")]
    Frame(Box<str>),
}

impl From<Error> for JsValue {
    fn from(e: Error) -> Self {
        JsValue::from_str(&e.to_string())
    }
}

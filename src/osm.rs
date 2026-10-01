use gloo_net::http::Request;
use serde::Deserialize;
use wasm_bindgen::JsValue;

const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";
const SDU_BBOX: (f64, f64, f64, f64) = (55.36269, 10.4220, 55.371464, 10.4342);

// Center of the bbox - the point we treat as local (0, 0).
const ORIGIN_LAT: f64 = (SDU_BBOX.0 + SDU_BBOX.2) / 2.0;
const ORIGIN_LON: f64 = (SDU_BBOX.1 + SDU_BBOX.3) / 2.0;

// Rough scale so the bbox's extent lands near [-1, 1]
const SCALE: f64 = 220.0;

#[derive(Debug, Deserialize)]
struct OverpassResponse {
    elements: Vec<Element>,
}

#[derive(Debug, Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    geometry: Vec<GeomPoint>,
}

#[derive(Debug, Deserialize)]
struct GeomPoint {
    lat: f64,
    lon: f64,
}

/// A single OSM way, reduced to just the points we care about for rendering.
#[derive(Debug, Clone)]
pub struct Way {
    pub points: Vec<(f64, f64)>, // (lat, lon)
}

fn err(msg: impl AsRef<str>) -> JsValue {
    JsValue::from_str(msg.as_ref())
}

fn project(lat: f64, lon: f64) -> (f32, f32) {
    let x = (lon - ORIGIN_LON) * ORIGIN_LAT.to_radians().cos() * SCALE;
    let y = (lat - ORIGIN_LAT) * SCALE;
    (x as f32, y as f32)
}

pub fn flatten_points(ways: &[Way]) -> Vec<f32> {
    let mut result: Vec<f32> = Vec::new();
    for way in ways {
        for &(lat, lon) in &way.points {
            let (x, y) = project(lat, lon);
            result.push(x);
            result.push(y);
        }
    }
    result
}

/// Sends a raw Overpass QL query and returns the parsed response
/// Generalized to fetch from any bounding box
async fn fetch_overpass(query: &str) -> Result<OverpassResponse, JsValue> {
    let request = Request::post(OVERPASS_URL)
        .header("Content-Type", "text/plain")
        .body(query.to_string())
        .map_err(|e| err(e.to_string()))?;

    let response = request.send().await.map_err(|e| err(e.to_string()))?;

    if !response.ok() {
        return Err(err(format!(
            "overpass request failed: HTTP {}",
            response.status()
        )));
    }

    response
        .json::<OverpassResponse>()
        .await
        .map_err(|e| err(e.to_string()))
}

/// Builds an Overpass query for roads/paths within a bounding box
/// `out geom;` makes Overpass attach lat/lon to every point of every way
/// directly, so we don't have to resolve node ids ourselves.
fn build_query(bbox: (f64, f64, f64, f64)) -> String {
    let (south, west, north, east) = bbox;
    format!("[out:json];way[building]({south},{west},{north},{east});out geom;")
}

/// Fetch road and paths for SDU and returns as ways
pub async fn fetch_sdu_map_data() -> Result<Vec<Way>, JsValue> {
    let query = build_query(SDU_BBOX);
    let response = fetch_overpass(&query).await?;

    let ways = response
        .elements
        .into_iter()
        .filter(|e| e.kind == "way")
        .map(|e| Way {
            points: e.geometry.into_iter().map(|p| (p.lat, p.lon)).collect(),
        })
        .collect();

    Ok(ways)
}

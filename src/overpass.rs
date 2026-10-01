use gloo_net::http::Request;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::geo::{Bbox, LatLon, Way};

const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";

#[derive(Deserialize)]
struct Response {
    elements: Box<[Element]>,
}

#[derive(Deserialize)]
struct Element {
    #[serde(rename = "type")]
    kind: Box<str>,
    #[serde(default)]
    geometry: Box<[LatLon]>,
}

fn buildings_query(b: &Bbox) -> String {
    let Bbox {
        south,
        west,
        north,
        east,
    } = b;
    format!("[out:json];way[building]({south},{west},{north},{east});out geom;")
}

pub async fn fetch_buildings(bbox: &Bbox) -> Result<Box<[Way]>> {
    let response = Request::post(OVERPASS_URL)
        .header("Content-Type", "text/plain")
        .body(buildings_query(bbox))?
        .send()
        .await?;

    if !response.ok() {
        return Err(Error::Http(response.status()));
    }

    let parsed: Response = response.json().await?;
    Ok(parsed
        .elements
        .into_vec()
        .into_iter()
        .filter(|e| &*e.kind == "way" && e.geometry.len() >= 2)
        .map(|e| Way { points: e.geometry })
        .collect())
}

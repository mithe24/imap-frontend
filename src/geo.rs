use serde::Deserialize;

const EARTH_RADIUS_M: f64 = 6_371_000.0;

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct LatLon {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Bbox {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl Bbox {
    pub fn center(&self) -> LatLon {
        LatLon {
            lat: (self.south + self.north) / 2.0,
            lon: (self.west + self.east) / 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Way {
    pub points: Box<[LatLon]>,
}

#[derive(Debug, Clone, Copy)]
pub struct Projection {
    origin: LatLon,
    cos_lat: f64,
}

impl Projection {
    pub fn centered_on(origin: LatLon) -> Self {
        Self {
            origin,
            cos_lat: origin.lat.to_radians().cos(),
        }
    }

    pub fn project(&self, p: LatLon) -> [f32; 2] {
        let x = (p.lon - self.origin.lon).to_radians()
            * self.cos_lat
            * EARTH_RADIUS_M;
        let y = (p.lat - self.origin.lat).to_radians() * EARTH_RADIUS_M;
        [x as f32, y as f32]
    }
}

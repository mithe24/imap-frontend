use bytemuck::{Pod, Zeroable};

use crate::mesh::Bounds;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ViewUniform {
    center: [f32; 2],
    scale: [f32; 2],
}

/// `zoom` is CSS pixels per world unit (meters).
/// `viewport` is in CSS pixels.
#[derive(Debug)]
pub struct Camera {
    center: [f32; 2],
    zoom: f32,
    zoom_range: (f32, f32),
    viewport: [f32; 2],
}

impl Camera {
    const FIT_MARGIN: f32 = 0.9;

    pub fn fit(bounds: Bounds, viewport: [f32; 2]) -> Self {
        let [w, h] = bounds.size();
        let fit = (viewport[0] / w.max(f32::EPSILON))
            .min(viewport[1] / h.max(f32::EPSILON))
            * Self::FIT_MARGIN;

        Self {
            center: bounds.center(),
            zoom: fit,
            zoom_range: (fit * 0.25, fit * 500.0),
            viewport,
        }
    }

    pub fn resize(&mut self, viewport: [f32; 2]) {
        self.viewport = viewport;
    }

    pub fn pan(&mut self, dx_px: f32, dy_px: f32) {
        self.center[0] -= dx_px / self.zoom;
        self.center[1] -= dy_px / self.zoom;
    }

    pub fn zoom_at(&mut self, cursor_px: [f32; 2], factor: f32) {
        let before = self.screen_to_world(cursor_px);
        self.zoom =
            (self.zoom * factor).clamp(self.zoom_range.0, self.zoom_range.1);
        let after = self.screen_to_world(cursor_px);
        self.center[0] += before[0] - after[0];
        self.center[1] += before[1] - after[1];
    }

    pub fn uniform(&self) -> ViewUniform {
        ViewUniform {
            center: self.center,
            scale: [
                2.0 * self.zoom / self.viewport[0],
                2.0 * self.zoom / self.viewport[1],
            ],
        }
    }

    pub fn screen_to_world(&self, p: [f32; 2]) -> [f32; 2] {
        [
            self.center[0] + (p[0] - self.viewport[0] / 2.0) / self.zoom,
            self.center[1] + (p[1] - self.viewport[1] / 2.0) / self.zoom,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_cursor_anchored() {
        let mut cam = Camera {
            center: [10.0, 20.0],
            zoom: 2.0,
            zoom_range: (0.1, 100.0),
            viewport: [800.0, 600.0],
        };
        let cursor = [123.0, 456.0];
        let before = cam.screen_to_world(cursor);
        cam.zoom_at(cursor, 1.7);
        let after = cam.screen_to_world(cursor);
        assert!((before[0] - after[0]).abs() < 1e-3);
        assert!((before[1] - after[1]).abs() < 1e-3);
    }
}

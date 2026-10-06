use bytemuck::{Pod, Zeroable};

use crate::{
    error::{Error::EmptyMap, Result},
    geo::{Projection, Way},
};

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 2],
}

#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub min: [f32; 2],
    pub max: [f32; 2],
}

impl Bounds {
    pub fn size(&self) -> [f32; 2] {
        [self.max[0] - self.min[0], self.max[1] - self.min[1]]
    }

    pub fn center(&self) -> [f32; 2] {
        [
            (self.min[0] + self.max[0]) / 2.0,
            (self.min[1] + self.max[1]) / 2.0,
        ]
    }
}

#[derive(Debug)]
pub struct Mesh {
    pub vertices: Box<[Vertex]>,
    pub indices: Box<[u32]>,
    pub bounds: Bounds,
}

impl Mesh {
    pub fn build(ways: &[Way], projection: &Projection) -> Result<Self> {
        let vertices: Box<[Vertex]> = ways
            .iter()
            .flat_map(|way| way.points.iter())
            .map(|&p| Vertex {
                position: projection.project(p),
            })
            .collect();

        if vertices.is_empty() {
            return Err(EmptyMap);
        }

        let mut base = 0u32;
        let indices = ways
            .iter()
            .flat_map(|way| {
                let start = base;
                let len = way.points.len() as u32;
                base += len;
                (1..len).flat_map(move |i| [start + i - 1, start + i])
            })
            .collect();

        let bounds = vertices.iter().fold(
            Bounds {
                min: [f32::MAX; 2],
                max: [f32::MIN; 2],
            },
            |b, v| Bounds {
                min: [b.min[0].min(v.position[0]), b.min[1].min(v.position[1])],
                max: [b.max[0].max(v.position[0]), b.max[1].max(v.position[1])],
            },
        );

        Ok(Self {
            vertices,
            indices,
            bounds,
        })
    }
}

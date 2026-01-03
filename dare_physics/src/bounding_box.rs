use bevy_ecs::prelude::*;

/// Describes a bounding box and a min max property in 3d space
#[derive(Debug, Clone, PartialEq, Default, Component)]
pub struct BoundingBox {
    min: glam::Vec3,
    max: glam::Vec3,
}

impl BoundingBox {
    /// Correct the existing bounding box ensuring minimum bounds are minimum and maximum bounds are maximum extents
    pub fn correct(&mut self) {
        let min = self.min.clone();
        let max = self.max.clone();
        self.min = min.min(max);
        self.max = max.max(min);
    }

    /// Retrieve the 6 planes making up the bounding box
    pub fn planes(&self) -> [crate::Plane; 6] {
        [
            crate::Plane {
                normal: glam::Vec3::new(1.0, 0.0, 0.0),
                distance: self.max.x,
            },
            crate::Plane {
                normal: glam::Vec3::new(-1.0, 0.0, 0.0),
                distance: -self.min.x,
            },
            crate::Plane {
                normal: glam::Vec3::new(0.0, 1.0, 0.0),
                distance: self.max.y,
            },
            crate::Plane {
                normal: glam::Vec3::new(0.0, -1.0, 0.0),
                distance: -self.min.y,
            },
            crate::Plane {
                normal: glam::Vec3::new(0.0, 0.0, 1.0),
                distance: self.max.z,
            },
            crate::Plane {
                normal: glam::Vec3::new(0.0, 0.0, -1.0),
                distance: -self.min.z,
            },
        ]
    }
}

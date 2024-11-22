use std::ops::Bound;
pub use super::super::prelude::components;
use bevy_ecs::prelude as becs;
use dagal::allocators::Allocator;
use glam::Vec4Swizzles;

#[derive(Debug, Clone, PartialEq, becs::Component)]
pub struct BoundingBox {
    pub min: glam::Vec3,
    pub max: glam::Vec3,
}
impl Eq for BoundingBox {}

impl BoundingBox {
    /// Given 2 vectors, it will automatically determine the bounding boxes of the 2
    pub fn new(v1: glam::Vec3, v2: glam::Vec3) -> Self {
        Self {
            min: v1.min(v2),
            max: v2.max(v1),
        }
    }

    /// Checks if the bounds are correct on the bounding box
    ///
    /// Checks if min(low, upper) == low
    pub fn is_bounds_correct(&self) -> bool {
        self.min.min(self.max) == self.min
    }

    /// Correct the bounding box to the correct min and max extents
    pub fn correct_bounding_box(&mut self) {
        let min = self.min.min(self.max);
        let max = self.max.max(self.min);
        self.min = min;
        self.max = max;
    }

    pub fn visible_in_frustum(&self, model_transform: glam::Mat4, view_proj: glam::Mat4) -> bool {
        let cube = [
            glam::Vec3::new(self.min.x, self.min.y, self.min.z),
            glam::Vec3::new(self.min.x, self.min.y, self.max.z),
            glam::Vec3::new(self.min.x, self.max.y, self.min.z),
            glam::Vec3::new(self.min.x, self.max.y, self.max.z),
            glam::Vec3::new(self.max.x, self.min.y, self.min.z),
            glam::Vec3::new(self.max.x, self.min.y, self.max.z),
            glam::Vec3::new(self.max.x, self.max.y, self.min.z),
            glam::Vec3::new(self.max.x, self.max.y, self.max.z),
        ];

        let matrix = view_proj * model_transform;
        let mut min = glam::Vec3::splat(1.5);
        let mut max = glam::Vec3::splat(-1.5);

        for vertex in cube.into_iter() {
            let mut v = matrix * glam::Vec4::from((vertex, 1.0));
            v.x /= v.w;
            v.y /= v.w;
            v.z /= v.w;

            min = min.min(v.xyz());
            max = max.max(v.xyz());
        }

        !(min.z > 1.0 || max.z < 0.0 || min.x > 1.0 || max.x < -1.0 || min.y > 1.0 || max.y < -1.0)
    }
}

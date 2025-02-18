use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use derivative::Derivative;

#[derive(becs::Bundle, Clone, Debug, PartialEq, Eq, Derivative)]
#[derivative(PartialOrd, Ord)]
pub struct Mesh {
    pub surface: super::Surface,
    #[derivative(PartialOrd = "ignore", Ord = "ignore")]
    pub material: super::Material,
    #[derivative(PartialOrd = "ignore", Ord = "ignore")]
    pub bounding_box: dare::render::components::bounding_box::BoundingBox,
    #[derivative(PartialOrd = "ignore", Ord = "ignore")]
    pub name: dare::engine::components::Name,
    #[derivative(PartialOrd = "ignore", Ord = "ignore")]
    pub transform: dare::physics::components::Transform,
}

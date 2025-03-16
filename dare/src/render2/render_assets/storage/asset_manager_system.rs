use crate::prelude as dare;
use bevy_ecs::prelude::*;
use dagal::allocators::{GPUAllocatorImpl, MemoryLocation};
use crate::render2::physical_resource;

pub fn asset_manager_system(
    mut physical_buffer: ResMut<
        physical_resource::PhysicalResourceStorage<physical_resource::RenderBuffer<GPUAllocatorImpl>>,
    >,
    mut physical_image: ResMut<
        physical_resource::PhysicalResourceStorage<physical_resource::RenderImage<GPUAllocatorImpl>>,
    >
) {
    physical_buffer.update();
    physical_image.update();
}

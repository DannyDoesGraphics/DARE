use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use dagal::allocators::GPUAllocatorImpl;
use futures::future;

/// Responsible for updating frame buffers
pub fn update_frame_buffer(
    // run time
    rt: becs::Res<'_, dare::concurrent::BevyTokioRunTime>,
    // Newly added surfaces
    surface_added_query: becs::Query<
        (
            becs::Entity,
            &dare::engine::components::Surface,
            &dare::physics::components::Transform,
            &dare::render::components::BoundingBox,
        ),
        becs::Added<dare::engine::components::Surface>,
    >,
    surface_changed_query: becs::Query<
        (
            becs::Entity,
            &dare::engine::components::Surface,
            &dare::physics::components::Transform,
            &dare::render::components::BoundingBox,
        ),
        becs::Changed<dare::engine::components::Surface>,
    >,
    mut surface_removed: becs::RemovedComponents<dare::engine::components::Surface>,
    // Frame resource
    mut window_context: becs::ResMut<'_, crate::render::contexts::WindowContext>,
    transfer_context: becs::Res<'_, crate::render::contexts::TransferContext>,
    // resources
    mut buffers: becs::ResMut<
        '_,
        dare::render::physical_resource::PhysicalResourceStorage<
            dare::render::physical_resource::RenderBuffer<GPUAllocatorImpl>,
        >,
    >,
) {
    rt.clone().runtime.block_on(async {
        // Batch update physical resources first for better performance
        let update_span = tracy_client::span!("buffers_update");
        buffers.update();
        update_span.emit_text("Buffer updates processed");

        // Check if we have a surface context
        if window_context.surface_context.is_none() {
            return;
        }

        // first process all additions, removals, and updates to surfaces and reflect said changes
        let mut deltas: Vec<dare::render::util::PersistentDelta<dare::render::c::CSurface>> =
            Vec::new();

        for (entity, surface, transform, bounding_box) in surface_added_query {
            let surface = dare::render::c::CSurface::from_surface_zero(
                &mut buffers,
                surface,
                transform,
                bounding_box,
            );
            println!("Added surface: {:?}", surface);
            deltas.push(dare::render::util::PersistentDelta::Added(
                entity.to_bits(),
                surface,
            ));
        }

        for (entity, surface, transform, bounding_box) in surface_changed_query {
            deltas.push(dare::render::util::PersistentDelta::Updated(
                entity.to_bits(),
                dare::render::c::CSurface::from_surface_zero(
                    &mut buffers,
                    surface,
                    transform,
                    bounding_box,
                ),
            ));
        }

        for entity in surface_removed.read() {
            deltas.push(dare::render::util::PersistentDelta::Removed(
                entity.to_bits(),
            ))
        }
        // Submit deltas
        for frame in window_context
            .surface_context
            .as_mut()
            .unwrap()
            .frames
            .iter_mut()
        {
            frame.surface_buffer_2.submit_queue(deltas.clone());
        }

        // Flush deltas in parallel
        if let Some(surface_context) = window_context.surface_context.as_mut() {
            let futures = surface_context
                .frames
                .iter_mut()
                .filter_map(|frame| {
                    // don't use this frame if it is currently rendering
                    if let Ok(fence_status) = frame.render_fence.get_fence_status() {
                        if !fence_status {
                            return None;
                        }
                    }
                    Some(
                        frame
                            .surface_buffer_2
                            .flush_queue(&transfer_context.immediate_submit),
                    )
                })
                .collect::<Vec<_>>();

            future::join_all(futures).await;
        }
    });
}

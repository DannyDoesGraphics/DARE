use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use futures::task::LocalSpawnExt;

pub fn render_server_shutdown_system(
    device_context: becs::Res<'_, crate::render2::contexts::DeviceContext>,
    window_context: becs::Res<'_, crate::render2::contexts::WindowContext>,
    rt: becs::Res<'_, dare::concurrent::BevyTokioRunTime>,
) {
    unsafe {
        device_context
            .device
            .get_handle()
            .device_wait_idle()
            .unwrap();
    }
    rt.runtime.block_on(async {
        if let Some(surface_context) = &window_context.surface_context {
            for frame in surface_context.frames.as_ref() {
                if frame.render_fence.get_fence_status().unwrap_or(true) == true {
                    continue;
                }
                frame.render_fence.wait(u64::MAX).unwrap();
            }
        }
    });
}

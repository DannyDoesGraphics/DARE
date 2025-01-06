use crate::prelude as dare;
use bevy_ecs::prelude as becs;
use futures::task::LocalSpawnExt;

pub fn render_server_shutdown_system(
    render_context: becs::Res<'_, dare::render::contexts::RenderContext>,
    rt: becs::Res<'_, dare::concurrent::BevyTokioRunTime>,
) {
    unsafe {
        render_context
            .inner
            .device
            .get_handle()
            .device_wait_idle()
            .unwrap();
    }
    rt.runtime.block_on(async {
        let binding = render_context.clone();
        let surface_context_guard = binding.inner.window_context.surface_context.read().unwrap();
        if let Some(surface_context) = &*surface_context_guard {
            for frame_mutex in surface_context.frames.as_ref() {
                let frame_guard = frame_mutex.lock().await;
                if frame_guard.render_fence.get_fence_status().unwrap_or(true) == true {
                    continue;
                }
                frame_guard.render_fence.wait(u64::MAX).unwrap();
            }
        }
    });
}

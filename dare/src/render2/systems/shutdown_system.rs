use crate::prelude as dare;
use bevy_ecs::prelude as becs;

pub fn render_server_shutdown_system(render_context: becs::Res<'_, dare::render::contexts::RenderContext>) {
    println!("Shutting down!");
        unsafe {
            render_context.inner.device.get_handle()
                          .device_wait_idle()
                .unwrap();
        }
}
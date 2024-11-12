use crate::prelude as dare;
use crate::render2::server::IrSend;
use bevy_ecs::prelude as becs;
use dagal::allocators::{GPUAllocatorImpl, MemoryLocation};
use dagal::ash::vk;

pub fn init_assets(
    mut commands: becs::Commands,
    rt: becs::Res<dare::concurrent::BevyTokioRunTime>,
    asset_server: becs::Res<dare::asset2::server::AssetServer>,
    send: becs::Res<IrSend>,
) {
    rt.runtime.block_on(async move {
        crate::asset2::gltf::GLTFLoader::load(
            &mut commands,
            &asset_server,
            send.clone(),
            std::path::PathBuf::from(
                "C:/Users/Danny/Documents/glTF-Sample-Models/2.0/Box/glTF/Box.gltf",
                //"C:/Users/Danny/Documents/glTF-Sample-Models/2.0/Sponza/glTF/Sponza.gltf",
                //"C:/Users/Danny/Documents/glTF-Sample-Models/2.0/Suzanne/glTF/Suzanne.gltf",
            ),
        )
        .unwrap();
    });
}

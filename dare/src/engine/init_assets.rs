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
                //"C:/Users/Danny/Documents/glTF-Sample-Models/2.0/Box/glTF/Box.gltf",
                //"C:/Users/Danny/Documents/glTF-Sample-Models/2.0/Sponza/glTF/Sponza.gltf",
                //"C:/Users/Danny/Documents/main1_sponza/main1_sponza/NewSponza_Main_glTF_003.gltf",
                "C:/Users/Danny/Documents/bistro/5_2/bistro_5_2.gltf",
                //"C:/Users/danny/Downloads/deccer-cubes-main/deccer-cubes-main/SM_Deccer_Cubes.gltf",
                //"C:/Users/Danny/Documents/Assets/junk_shop/Blender.gltf",
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/Sponza/glTF/Sponza.gltf"
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/Suzanne/glTF/Suzanne.gltf"
                //"C:/Users/Danny/Documents/glTF-Sample-Models/2.0/Suzanne/glTF/Suzanne.gltf",
                //"C:/Users/Danny/Documents/glTF-Sample-Models/2.0/DamagedHelmet/glTF/DamagedHelmet.gltf",
                //"C:/Users/Danny/Documents/glTF-Sample-Models/2.0/Lantern/glTF/Lantern.gltf",
            ),
        )
        .unwrap();
    });
}

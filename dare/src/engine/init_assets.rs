use crate::prelude as dare;
use bevy_ecs::prelude as becs;

pub fn init_assets(
    mut commands: becs::Commands,
    rt: becs::Res<dare::concurrent::BevyTokioRunTime>,
    asset_server: becs::Res<dare::asset::server::AssetServer>,
) {
    rt.runtime.block_on(async move {
        crate::asset::gltf::GLTFLoader::load(
            &mut commands,
            &asset_server,
            std::path::PathBuf::from(
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/Box/glTF/Box.gltf",
                //"C:/Users/Danny/Documents/main1_sponza/main1_sponza/NewSponza_Main_glTF_003.gltf",
                "C:/Users/Danny/Documents/bistro/5_2/bistro_5_2.gltf",
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/BoxTextured/glTF/BoxTextured.gltf",
                //"C:/Users/danny/Documents/blender_splashes/test/instances/instances.gltf",
                //"C:/Users/danny/Downloads/deccer-cubes-main/deccer-cubes-main/SM_Deccer_Cubes.gltf",
                //"C:/Users/Danny/Documents/Assets/junk_shop/Blender.gltf",
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/Sponza/glTF/Sponza.gltf"
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/Suzanne/glTF/Suzanne.gltf",
                //"C:/Users/Danny/Documents/glTF-Sample-Assets-main/Models/DamagedHelmet/glTF/DamagedHelmet.gltf",
                //"C:/Users/Danny/Documents/glTF-Sample-Assets-main/Models/Lantern/glTF/Lantern.gltf",
                //"C:/Users/danny/Documents/tests/2_of_us/2_of_us.gltf",
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/Lantern/glTF/Lantern.gltf",
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/Box/glTF/Box.gltf",
                //"C:/Users/danny/Documents/glTF-Sample-Assets-main/Models/2CylinderEngine/glTF/2CylinderEngine.gltf"
            ),
        )
        .unwrap();
    });
}

use bevy_ecs::prelude::*;

pub fn init_assets(mut commands: Commands, mut asset_system: ResMut<dare_assets::AssetManager>) {
    asset_system.load_gltf(
        &mut commands,
        &std::path::PathBuf::from("C:/Users/Danny/Documents/bistro/5_2/bistro_5_2.gltf"),
    );
}

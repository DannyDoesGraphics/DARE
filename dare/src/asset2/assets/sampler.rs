pub struct ImageSampler {}

#[derive(PartialEq, Eq, Clone)]
pub struct SamplerAsset {
    wrapping_mode: (
        gltf::json::texture::WrappingMode,
        gltf::json::texture::WrappingMode,
    ),
    min_filter: gltf::texture::MinFilter,
    mag_filter: gltf::texture::MagFilter,
}

#[repr(C)]
#[derive(Debug)]
pub struct RasterizationPushConstant {
    pub scene_data: u64,
    pub surface_data: u64,
    pub model_transform: glam::Mat4,
}

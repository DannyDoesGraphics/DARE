#[repr(C)]
#[derive(Debug)]
pub struct SceneData {
    pub view: glam::Mat4,
    pub proj: glam::Mat4,
    pub view_proj: glam::Mat4,
}
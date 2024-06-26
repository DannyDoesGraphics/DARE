#[repr(C)]
#[derive(Debug)]
pub struct SceneData {
    pub view: [f32; 16],
    pub proj: [f32; 16],
    pub view_proj: [f32; 16],
}

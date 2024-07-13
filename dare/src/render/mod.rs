pub use gpu_resource::GPUResource;
pub use material::*;
pub use mesh::*;
pub use surface::*;
pub use texture::*;

pub mod allocators;
pub mod camera;
pub mod draw_context;
/// Deals with primitives relating to rendering
///
/// The renderer reads from the render primitives to determine what needs to be read
pub mod material;
pub mod pipeline;
pub mod push_constants;
pub mod render_context;
pub mod scene_data;
pub mod surface;
pub mod texture;
pub mod image;
pub mod acceleration_structure;
pub mod growable_buffer;
pub mod deferred_deletion;
mod backed_growable;
pub mod backable;
pub mod mesh;
pub mod gpu_resource;

pub use material::*;
pub use mesh::*;
pub use surface::*;
pub use texture::*;

pub mod acceleration_structure;
pub mod allocators;
pub mod backable;
mod backed_growable;
pub mod camera;
pub mod deferred_deletion;
pub mod draw_context;
pub mod gpu_resource;
pub mod growable_buffer;
pub mod image;
/// Deals with primitives relating to rendering
///
/// The renderer reads from the render primitives to determine what needs to be read
pub mod material;
pub mod mesh;
mod mesh2;
pub mod pipeline;
pub mod push_constants;
pub mod render_context;
pub mod render_system;
pub mod scene_data;
pub mod surface;
pub mod texture;
pub mod transfer;

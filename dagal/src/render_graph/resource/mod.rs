pub(crate) mod storage;

use crate::DefaultAllocator;
use ash::vk;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

/// Represents the load state of a resource in the render graph
#[derive(Debug)]
pub(crate) enum LoadState<T: ResourceMetadata> {
    Unloaded,
    Loading,
    Loaded {
        /// Physical representation of the resource
        resource: T::Physical,
        /// State of the resource, which can be used to track its lifecycle
        state: T::State,
    },
}
impl<T: ResourceMetadata> LoadState<T> {
    pub fn is_loaded(&self) -> bool {
        matches!(self, LoadState::Loaded { .. })
    }

    pub fn is_unloaded(&self) -> bool {
        matches!(self, LoadState::Unloaded)
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, LoadState::Loading)
    }

    /// Apply a function to the loaded resource and state, returning the result wrapped in Some
    pub fn and_then<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T::Physical, &T::State) -> R,
    {
        match self {
            LoadState::Loaded { resource, state } => Some(f(resource, state)),
            _ => None,
        }
    }

    /// Apply a mutable function to the loaded resource and state
    pub fn and_then_mut<F, R>(&mut self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T::Physical, &mut T::State) -> R,
    {
        match self {
            LoadState::Loaded { resource, state } => Some(f(resource, state)),
            _ => None,
        }
    }

    /// Replace the loaded resource with a new one, returning the old resource and state
    pub fn replace(
        &mut self,
        new_resource: T::Physical,
        new_state: T::State,
    ) -> Option<(T::Physical, T::State)> {
        match self {
            LoadState::Loaded { .. } => {
                let old_value = std::mem::replace(
                    self,
                    LoadState::Loaded {
                        resource: new_resource,
                        state: new_state,
                    },
                );
                match old_value {
                    LoadState::Loaded { resource, state } => Some((resource, state)),
                    _ => unreachable!("Failed to swap states"),
                }
            }
            _ => {
                // Set to loaded state even if it wasn't loaded before
                *self = LoadState::Loaded {
                    resource: new_resource,
                    state: new_state,
                };
                None
            }
        }
    }

    /// Take the resource and state, leaving the LoadState as Unloaded
    pub fn take(&mut self) -> Option<(T::Physical, T::State)> {
        match std::mem::replace(self, LoadState::Unloaded) {
            LoadState::Loaded { resource, state } => Some((resource, state)),
            _ => None,
        }
    }

    /// Take only the physical resource, leaving the LoadState as Unloaded
    pub fn take_resource(&mut self) -> Option<T::Physical> {
        self.take().map(|(resource, _)| resource)
    }

    /// Get a reference to the physical resource if loaded
    pub fn get(&self) -> Option<&T::Physical> {
        match self {
            LoadState::Loaded { resource, .. } => Some(resource),
            _ => None,
        }
    }

    /// Get a mutable reference to the physical resource if loaded
    pub fn get_mut(&mut self) -> Option<&mut T::Physical> {
        match self {
            LoadState::Loaded { resource, .. } => Some(resource),
            _ => None,
        }
    }

    /// Get a reference to the state if loaded
    pub fn get_state(&self) -> Option<&T::State> {
        match self {
            LoadState::Loaded { state, .. } => Some(state),
            _ => None,
        }
    }

    /// Get a mutable reference to the state if loaded
    pub fn get_state_mut(&mut self) -> Option<&mut T::State> {
        match self {
            LoadState::Loaded { state, .. } => Some(state),
            _ => None,
        }
    }

    /// Get references to both resource and state if loaded
    pub fn get_both(&self) -> Option<(&T::Physical, &T::State)> {
        match self {
            LoadState::Loaded { resource, state } => Some((resource, state)),
            _ => None,
        }
    }

    /// Get mutable references to both resource and state if loaded
    pub fn get_both_mut(&mut self) -> Option<(&mut T::Physical, &mut T::State)> {
        match self {
            LoadState::Loaded { resource, state } => Some((resource, state)),
            _ => None,
        }
    }

    /// Map the physical resource if loaded
    pub fn map_resource<F>(self, f: F) -> LoadState<T>
    where
        F: FnOnce(T::Physical) -> T::Physical,
    {
        match self {
            LoadState::Loaded { resource, state } => LoadState::Loaded {
                resource: f(resource),
                state,
            },
            other => other,
        }
    }

    /// Map the state if loaded
    pub fn map_state<F>(self, f: F) -> LoadState<T>
    where
        F: FnOnce(T::State) -> T::State,
    {
        match self {
            LoadState::Loaded { resource, state } => LoadState::Loaded {
                resource,
                state: f(state),
            },
            other => other,
        }
    }
}

pub(crate) trait ResourceMetadata: Debug {
    /// State of the resource
    type State: Debug;

    /// Physical representation of the resource
    type Physical: Debug;
}

/// Storage entry for a resource in the render graph
#[derive(Debug)]
pub(crate) struct Resource<Metadata: ResourceMetadata> {
    metadata: Metadata,
    state: Metadata::State,
    physical: LoadState<Metadata>,
}

#[derive(PartialEq, Debug, Clone)]
pub enum TextureExtents {
    /// Custom extents for the texture
    Custom(vk::Extent3D),
    /// Uses the resolution for the texture extents
    FullResolution,
    /// Uses a multiple of the resolution
    ///
    /// Note: Will round to the nearest integer
    MultiplierResolution(f32),
}

impl Eq for TextureExtents {}
impl Hash for TextureExtents {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            TextureExtents::Custom(extent) => {
                0.hash(state);
                extent.hash(state);
            }
            TextureExtents::FullResolution => {
                1.hash(state);
            }
            TextureExtents::MultiplierResolution(mul) => {
                2.hash(state);
                mul.to_bits().hash(state);
            }
        }
    }
}

/// Defines a virtual resource for textures
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureVirtualResource {
    /// Optional name for the texture
    name: Option<String>,
    /// Usage flags for the texture
    usage_flags: vk::ImageUsageFlags,
    /// The location of the texture in memory
    location: crate::allocators::MemoryLocation,
    /// The format of the texture
    format: vk::Format,
    /// The extents of the texture
    extents: TextureExtents,
    /// If the texture should persist across frames
    persistent: bool,
}

/// Defines a virtual resource for buffers
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BufferVirtualResource {
    /// Optional name for the buffer
    pub name: Option<String>,
    /// Usage flags for the buffer
    pub usage_flags: vk::BufferUsageFlags,
    /// Location of the buffer in memory
    pub location: crate::allocators::MemoryLocation,
    /// Size of the buffer in bytes
    pub size: vk::DeviceSize,
    /// If the buffer should persist across frames
    pub persistent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BufferPhysicalState {
    pub queue_family_index: u32,
}

impl ResourceMetadata for BufferVirtualResource {
    type State = BufferPhysicalState;

    type Physical = crate::resource::Buffer<DefaultAllocator>;
}

pub enum ResourceType {
    Texture(TextureVirtualResource),
    Buffer(BufferVirtualResource),
}

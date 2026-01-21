use bevy_ecs::prelude::*;

/// Similar to Bevy's implementation.
///
/// Marks an entity to be copied
#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Component)]
pub struct SyncToRenderWorld ();

#[derive(Debug, Copy, Clone, Component, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MainEntity(Entity);
impl MainEntity {
    #[inline]
    pub fn entity(&self) -> Entity {
        self.0
    }
}


#[derive(Debug, Copy, Clone, Component, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderEntity(Entity);
impl RenderEntity {
    #[inline]
    pub fn entity(&self) -> Entity {
        self.0
    }
}


/// Link between a sender and reciever
pub fn link_component<T: Component>(world_recv: &mut World, world_send: &mut World) {
    world_send.register_required_components::<T, SyncToRenderWorld>();
    
    
    
    /// Applied system on send world
    fn extract_components(world: &mut World) {
        
    }
    
    
}
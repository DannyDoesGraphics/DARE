use crate::prelude as dare;
use crate::prelude::render::{InnerRenderServerRequest, RenderServerAssetRelationDelta};
use crate::render2::server::IrSend;
use bevy_ecs::prelude as becs;

/// updates the server from any newly added surfaces from the engine world
pub fn engine_asset_sync_system(
    send: becs::Res<IrSend>,
    query: becs::Query<
        (
            &dare::engine::components::Surface,
            &dare::engine::components::name::Name,
            &dare::render::components::bounding_box::BoundingBox,
            &dare::physics::components::Transform,
            becs::Entity,
        ),
        becs::Added<dare::engine::components::Surface>,
    >,
) {
    for (surface, name, bounding_box, transform, entity) in query.iter() {
        send.0
            .send(InnerRenderServerRequest::Delta(
                RenderServerAssetRelationDelta::Entry(
                    entity,
                    dare::engine::components::Mesh {
                        surface: surface.clone(),
                        transform: transform.clone(),
                        name: name.clone(),
                        bounding_box: bounding_box.clone(),
                    },
                ),
            ))
            .unwrap();
    }
}

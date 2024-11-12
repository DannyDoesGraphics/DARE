use crate::prelude as dare;
use crate::prelude::render::{InnerRenderServerRequest, RenderServerAssetRelationDelta};
use crate::render2::server::IrSend;
use bevy_ecs::prelude as becs;

/// updates the server
pub fn asset_system(
    send: becs::Res<IrSend>,
    query: becs::Query<
        (&dare::engine::components::Surface, becs::Entity),
        becs::Added<dare::engine::components::Surface>,
    >,
) {
    for (surface, entity) in query.iter() {
        send.0
            .send(InnerRenderServerRequest::Delta(
                RenderServerAssetRelationDelta::Entry(entity, surface.clone()),
            ))
            .unwrap();
    }
}

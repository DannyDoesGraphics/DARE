use bevy_ecs::prelude::*;
use dare_assets::{AssetHandle, Mesh};
use dare_ecs::{App, AppStage, ExtractPlugin, Plugin, ProjectPlugin, SubAppMainLabel};
use dare_physics::{BoundingBox, Plane, Transform};
use dare_window::Window;
use glam::{Mat3, Mat4, Vec3};

use crate::plugin::{Camera, CameraPlugin, CameraUpdate, RenderMode, RenderSubAppLabel};

/// Entities (in `SubAppMainLabel`'s world) whose meshes survived culling this tick.
///
/// Resolve to render-world entities via `ProjectEntityMapping`; their
/// `AssetHandle<Mesh>` and `Transform` are projected alongside this list.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct VisibleMeshList(pub Vec<Entity>);

struct Frustum {
    planes: [Plane; 6],
}

impl Frustum {
    fn from_view_projection(view_projection: Mat4) -> Self {
        let row = |i| view_projection.row(i);
        let (r0, r1, r2, r3) = (row(0), row(1), row(2), row(3));

        let half_spaces = [
            r3 + r0, // left
            r3 - r0, // right
            r3 + r1, // bottom
            r3 - r1, // top
            r2,      // near (z >= 0)
            r3 - r2, // far
        ];

        Self {
            planes: half_spaces.map(|h| Plane {
                normal: h.truncate(),
                distance: -h.w,
            }),
        }
    }

    fn intersects_aabb(&self, min: Vec3, max: Vec3) -> bool {
        self.planes.iter().all(|plane| {
            let furthest = Vec3::select(plane.normal.cmpge(Vec3::ZERO), max, min);
            furthest.dot(plane.normal) - plane.distance >= 0.0
        })
    }
}

fn world_aabb(bounds: &BoundingBox, transform: &Transform) -> (Vec3, Vec3) {
    let matrix = transform.get_transform_matrix();
    let center = (bounds.min() + bounds.max()) * 0.5;
    let extent = (bounds.max() - bounds.min()) * 0.5;

    let world_center = matrix.transform_point3(center);
    let basis = Mat3::from_mat4(matrix);
    let abs_basis = Mat3::from_cols(basis.x_axis.abs(), basis.y_axis.abs(), basis.z_axis.abs());
    let world_extent = abs_basis * extent;

    (world_center - world_extent, world_center + world_extent)
}

#[derive(Default)]
pub struct CullPlugin;

impl Plugin for CullPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(CameraPlugin);

        app.add_plugin(ProjectPlugin::<
            SubAppMainLabel,
            RenderSubAppLabel,
            AssetHandle<Mesh>,
        >::new());
        app.add_plugin(ProjectPlugin::<SubAppMainLabel, RenderSubAppLabel, Transform>::new());

        app.world_mut().init_resource::<VisibleMeshList>();

        app.get_sub_app_mut::<RenderSubAppLabel>()
            .expect("CullPlugin requires the render sub-app")
            .world_mut()
            .init_resource::<VisibleMeshList>();

        app.schedule_scope(|schedule| {
            schedule.add_systems(cull.in_set(AppStage::Update).after(CameraUpdate));
        });

        app.add_plugin(ExtractPlugin::<
            VisibleMeshList,
            RenderSubAppLabel,
            SubAppMainLabel,
        >::from_cloneable_resource());
    }
}

fn cull(
    camera: Res<Camera>,
    mode: Res<RenderMode>,
    window: Res<Window>,
    meshes: Query<(Entity, &BoundingBox, &Transform), With<AssetHandle<Mesh>>>,
    mut visible: ResMut<VisibleMeshList>,
) {
    visible.0.clear();

    let Window::Window { physical_size, .. } = *window else {
        return;
    };
    let (width, height) = physical_size;
    if width == 0 || height == 0 {
        return;
    }

    // Path tracing skips frustum culling
    let frustum = mode.is_raster().then(|| {
        let aspect = width as f32 / height as f32;
        Frustum::from_view_projection(camera.projection(aspect) * camera.view())
    });

    visible
        .0
        .extend(meshes.iter().filter_map(|(entity, bounds, transform)| {
            if let Some(frustum) = &frustum {
                let (min, max) = world_aabb(bounds, transform);
                if !frustum.intersects_aabb(min, max) {
                    return None;
                }
            }
            Some(entity)
        }));
}

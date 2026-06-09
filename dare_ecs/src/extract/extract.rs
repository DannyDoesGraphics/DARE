use crate::extract::ExtractResource;
use crate::subapp::SubAppLabel;
use crate::{App, Plugin};
use bevy_ecs::prelude::*;
use std::marker::PhantomData;
use std::sync::Arc;

type ExtractFn<T> = Arc<dyn Fn(&mut World) -> Option<T> + Send + Sync>;
type ConsumeFn<T> = Arc<dyn Fn(&mut World, Vec<T>) + Send + Sync>;

/// Allows us to extract values from one subapp and consume them in another.
///
/// # Unified channel
/// In the background, we use [`ExtractResource<To, From>`] to allow multiple extract plugins to share the same channel.
/// This reduces the number of channels we need to manage. Another way to think of it is that each ExtractPlugin defines a
/// key-value pair in the map where the key is the type of the value we're extracting.
pub struct ExtractPlugin<T: Send + Sync + 'static, To: SubAppLabel, From: SubAppLabel> {
    _marker: PhantomData<(From, To, T)>,
    extract_fn: ExtractFn<T>,
    consume_fn: ConsumeFn<T>,
}

impl<T: Send + Sync + 'static, To: SubAppLabel, From: SubAppLabel> ExtractPlugin<T, To, From> {
    pub fn new(
        extract_fn: impl Fn(&mut World) -> Option<T> + Send + Sync + 'static,
        consume_fn: impl Fn(&mut World, Vec<T>) + Send + Sync + 'static,
    ) -> Self {
        Self {
            _marker: PhantomData,
            extract_fn: Arc::new(extract_fn),
            consume_fn: Arc::new(consume_fn),
        }
    }
}

impl<T: Send + Sync + 'static, To: SubAppLabel, From: SubAppLabel> Plugin
    for ExtractPlugin<T, To, From>
{
    fn build(&self, app: &mut App) {
        app.add_plugin(ExtractResource::<To, From>::default());

        let extract_fn = Arc::clone(&self.extract_fn);
        app.get_sub_app_mut::<From>()
            .unwrap()
            .schedule_scope(|schedule| {
                schedule.add_systems(
                    (move |world: &mut World| {
                        if let Some(value) = extract_fn(world) {
                            world
                                .get_resource_mut::<ExtractResource<To, From>>()
                                .unwrap()
                                .insert(value);
                        }
                    })
                    .in_set(crate::AppStage::PostUpdate),
                );
            });

        let consume_fn = Arc::clone(&self.consume_fn);
        app.get_sub_app_mut::<To>()
            .unwrap()
            .schedule_scope(|schedule| {
                schedule.add_systems(
                    (move |world: &mut World| {
                        let snapshots = world
                            .get_resource_mut::<ExtractResource<To, From>>()
                            .map(|mut extract| extract.take_snapshots::<T>())
                            .unwrap_or_default();
                        consume_fn(world, snapshots);
                    })
                    .in_set(crate::AppStage::PreUpdate),
                );
            });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{App, SubApp, SubAppLabel};
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct SimLabel;
    struct RenderLabel;
    impl SubAppLabel for SimLabel {}
    impl SubAppLabel for RenderLabel {}

    #[derive(Resource, Default)]
    struct Consumed<T: Send + Sync + 'static>(Option<Vec<T>>);

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct SimSnapshot {
        tick: u64,
        entity_count: u32,
    }

    fn run_pipeline<T: Send + Sync + 'static>(
        extract_fn: impl Fn(&mut World) -> Option<T> + Send + Sync + 'static,
        consume_fn: impl Fn(&mut World, Vec<T>) + Send + Sync + 'static,
    ) -> App {
        let mut app = App::new();
        app.insert_sub_app::<SimLabel>(SubApp::new());
        app.insert_sub_app::<RenderLabel>(SubApp::new());
        app.add_plugin(ExtractPlugin::<T, RenderLabel, SimLabel>::new(
            extract_fn, consume_fn,
        ));
        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();
        app
    }

    fn consumed<T: Send + Sync + 'static>(app: &mut App) -> Option<Vec<T>> {
        let world = app.get_sub_app_mut::<RenderLabel>().unwrap().world_mut();
        world.get_resource_mut::<Consumed<T>>()?.0.take()
    }

    #[derive(Resource)]
    struct ExtractSteps {
        values: Vec<Option<u32>>,
        index: AtomicUsize,
    }

    impl ExtractSteps {
        fn next(&self) -> Option<u32> {
            let i = self.index.fetch_add(1, Ordering::Relaxed);
            self.values.get(i).copied().flatten()
        }
    }

    fn tick_pair(app: &mut App) -> Option<Vec<u32>> {
        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();
        consumed(app)
    }

    #[test]
    fn multi_tick_none_then_some_matches_sequence() {
        let mut app = App::new();
        app.insert_sub_app::<SimLabel>(SubApp::new());
        app.insert_sub_app::<RenderLabel>(SubApp::new());
        app.get_sub_app_mut::<SimLabel>()
            .unwrap()
            .world_mut()
            .insert_resource(ExtractSteps {
                values: vec![None, Some(1), None, Some(2)],
                index: AtomicUsize::new(0),
            });
        app.add_plugin(ExtractPlugin::<u32, RenderLabel, SimLabel>::new(
            |world| world.resource::<ExtractSteps>().next(),
            |world, snapshots| {
                world.insert_resource(Consumed(Some(snapshots)));
            },
        ));

        assert_eq!(tick_pair(&mut app), Some(vec![]));
        assert_eq!(tick_pair(&mut app), Some(vec![1]));
        assert_eq!(tick_pair(&mut app), Some(vec![]));
        assert_eq!(tick_pair(&mut app), Some(vec![2]));
        assert_eq!(tick_pair(&mut app), Some(vec![]));
    }

    #[derive(Resource, Default)]
    struct ConsumeLog(Vec<Vec<u32>>);

    #[test]
    fn inconsistent_tick_rates_deliver_full_sim_sequence() {
        let mut app = App::new();
        app.insert_sub_app::<SimLabel>(SubApp::new());
        app.insert_sub_app::<RenderLabel>(SubApp::new());
        app.get_sub_app_mut::<SimLabel>()
            .unwrap()
            .world_mut()
            .insert_resource(ExtractSteps {
                values: vec![Some(1), Some(2), Some(3), Some(4)],
                index: AtomicUsize::new(0),
            });
        app.add_plugin(ExtractPlugin::<u32, RenderLabel, SimLabel>::new(
            |world| world.resource::<ExtractSteps>().next(),
            |world, snapshots| {
                world.get_resource_or_init::<ConsumeLog>().0.push(snapshots);
            },
        ));

        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();

        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();
        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();

        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();

        let log = app
            .get_sub_app_mut::<RenderLabel>()
            .unwrap()
            .world()
            .resource::<ConsumeLog>()
            .0
            .clone();

        assert_eq!(log, vec![vec![1, 2, 3], vec![], vec![], vec![4],]);
    }

    #[test]
    fn batches_multiple_sends_before_consume() {
        let mut app = App::new();
        app.insert_sub_app::<SimLabel>(SubApp::new());
        app.insert_sub_app::<RenderLabel>(SubApp::new());
        app.get_sub_app_mut::<SimLabel>()
            .unwrap()
            .world_mut()
            .insert_resource(ExtractSteps {
                values: vec![Some(1), Some(2), Some(3)],
                index: AtomicUsize::new(0),
            });
        app.add_plugin(ExtractPlugin::<u32, RenderLabel, SimLabel>::new(
            |world| world.resource::<ExtractSteps>().next(),
            |world, snapshots| {
                world.insert_resource(Consumed(Some(snapshots)));
            },
        ));

        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();

        assert_eq!(consumed::<u32>(&mut app), Some(vec![1, 2, 3]));
    }

    #[test]
    fn multiple_extract_types_share_one_transport() {
        let mut app = App::new();
        app.insert_sub_app::<SimLabel>(SubApp::new());
        app.insert_sub_app::<RenderLabel>(SubApp::new());
        app.add_plugin(ExtractPlugin::<u32, RenderLabel, SimLabel>::new(
            |_| Some(10u32),
            |world, snapshots| {
                world.insert_resource(Consumed(Some(snapshots)));
            },
        ));
        app.add_plugin(ExtractPlugin::<u64, RenderLabel, SimLabel>::new(
            |_| Some(20u64),
            |world, snapshots| {
                world.insert_resource(Consumed(Some(snapshots)));
            },
        ));

        app.get_sub_app_mut::<SimLabel>().unwrap().tick();
        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();

        assert_eq!(consumed::<u32>(&mut app), Some(vec![10]));
        assert_eq!(consumed::<u64>(&mut app), Some(vec![20]));
    }

    #[test]
    fn roundtrips_struct_payload() {
        let expected = SimSnapshot {
            tick: 7,
            entity_count: 128,
        };

        let mut app = run_pipeline(
            |_| {
                Some(SimSnapshot {
                    tick: 7,
                    entity_count: 128,
                })
            },
            |world, snapshots| {
                world.insert_resource(Consumed(Some(snapshots)));
            },
        );

        assert_eq!(consumed::<SimSnapshot>(&mut app), Some(vec![expected]));
    }

    #[test]
    fn roundtrips_vec_payload() {
        let mut app = run_pipeline(
            |_| Some(vec![10u32, 20, 30]),
            |world, snapshots| {
                world.insert_resource(Consumed(Some(snapshots)));
            },
        );

        assert_eq!(consumed::<Vec<u32>>(&mut app), Some(vec![vec![10, 20, 30]]));
    }

    #[test]
    fn skips_send_when_extract_returns_none() {
        let mut app = run_pipeline(
            |_| None::<SimSnapshot>,
            |world, snapshots| {
                world.insert_resource(Consumed(Some(snapshots)));
            },
        );

        assert_eq!(consumed::<SimSnapshot>(&mut app), Some(vec![]));
    }

    #[test]
    fn does_not_consume_without_from_tick() {
        let mut app = App::new();
        app.insert_sub_app::<SimLabel>(SubApp::new());
        app.insert_sub_app::<RenderLabel>(SubApp::new());
        app.add_plugin(ExtractPlugin::<Vec<u8>, RenderLabel, SimLabel>::new(
            |_| Some(vec![1, 2, 3]),
            |world, snapshots| {
                world.insert_resource(Consumed(Some(snapshots)));
            },
        ));

        app.get_sub_app_mut::<RenderLabel>().unwrap().tick();

        assert_eq!(consumed::<Vec<u8>>(&mut app), Some(vec![]));
    }
}

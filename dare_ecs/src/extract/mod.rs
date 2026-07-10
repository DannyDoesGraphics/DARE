mod extract;
mod project;
pub use extract::*;
pub use project::*;

use crate::subapp::SubAppLabel;
use crate::{App, AppStage, Plugin};
use bevy_ecs::prelude::*;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};

/// Maps a tuple of (From, To, T) to a value.
pub(crate) type ExtractMap = HashMap<(TypeId, TypeId, TypeId), Box<dyn Any + Send + Sync>>;

/// Handles channel extraction/consumption of the centralized hashmap for ExtractPlugins.
#[derive(Resource)]
pub(crate) struct ExtractResource<To: SubAppLabel, From: SubAppLabel> {
    pub(crate) map: ExtractMap,
    pub(crate) inbound: Vec<ExtractMap>,
    pub(crate) channel: dare_util::Either<
        crossbeam_channel::Sender<ExtractMap>,
        Arc<Mutex<crossbeam_channel::Receiver<ExtractMap>>>,
    >,
    _to: PhantomData<To>,
    _from: PhantomData<From>,
}

impl<To: SubAppLabel, From: SubAppLabel> Default for ExtractResource<To, From> {
    fn default() -> Self {
        let (send, _) = crossbeam_channel::unbounded();
        Self {
            map: HashMap::new(),
            inbound: Vec::new(),
            channel: dare_util::Either::Left(send),
            _to: PhantomData,
            _from: PhantomData,
        }
    }
}

impl<To: SubAppLabel, From: SubAppLabel> ExtractResource<To, From> {
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) {
        self.map.insert(
            (TypeId::of::<From>(), TypeId::of::<To>(), TypeId::of::<T>()),
            Box::new(value),
        );
    }

    pub fn send(&mut self) -> Result<(), crossbeam_channel::SendError<ExtractMap>> {
        if self.map.is_empty() {
            return Ok(());
        }
        let mut map = HashMap::new();
        std::mem::swap(&mut self.map, &mut map);
        self.channel.left_ref().unwrap().send(map)
    }

    pub fn drain_channel(&mut self) {
        let recv = self.channel.right_ref().unwrap();
        while let Ok(map) = recv.lock().unwrap().try_recv() {
            self.inbound.push(map);
        }
    }

    /// Remove `T` from each queued snapshot that contains it; drops empty maps.
    pub fn take_snapshots<T: Send + Sync + 'static>(&mut self) -> Vec<T> {
        let mut snapshots = Vec::new();
        self.inbound.retain_mut(|map| {
            if let Some(value) = map
                .remove(&(TypeId::of::<From>(), TypeId::of::<To>(), TypeId::of::<T>()))
                .and_then(|b| b.downcast::<T>().ok().map(|b| *b))
            {
                snapshots.push(value);
            }
            !map.is_empty()
        });
        snapshots
    }
}

impl<To: SubAppLabel, From: SubAppLabel> Plugin for ExtractResource<To, From> {
    fn build(&self, app: &mut App) {
        let (tx, recv) = crossbeam_channel::unbounded();

        app.get_sub_app_mut::<From>()
            .unwrap()
            .world_mut()
            .insert_resource(ExtractResource::<To, From> {
                map: HashMap::new(),
                inbound: Vec::new(),
                channel: dare_util::Either::Left(tx),
                _to: PhantomData,
                _from: PhantomData,
            });

        app.get_sub_app_mut::<From>()
            .unwrap()
            .schedule_scope(|schedule| {
                schedule.add_systems(
                    (|mut extract: ResMut<ExtractResource<To, From>>| {
                        let _ = extract.send();
                    })
                    .in_set(AppStage::Last),
                );
            });

        app.get_sub_app_mut::<To>()
            .unwrap()
            .world_mut()
            .insert_resource(ExtractResource::<To, From> {
                map: HashMap::new(),
                inbound: Vec::new(),
                channel: dare_util::Either::Right(Arc::new(Mutex::new(recv))),
                _to: PhantomData,
                _from: PhantomData,
            });

        app.get_sub_app_mut::<To>()
            .unwrap()
            .schedule_scope(|schedule| {
                schedule.add_systems(
                    (|mut extract: ResMut<ExtractResource<To, From>>| {
                        extract.drain_channel();
                    })
                    .in_set(AppStage::First),
                );
            });
    }

    fn cleanup(self: Box<Self>, app: &mut App) {
        if let Some(from) = app
            .get_sub_app_mut::<From>()
            .unwrap()
            .world_mut()
            .remove_resource::<ExtractResource<To, From>>()
        {
            app.get_sub_app_mut::<From>()
                .unwrap()
                .world_mut()
                .insert_resource(from);
        }
        if let Some(to) = app
            .get_sub_app_mut::<To>()
            .unwrap()
            .world_mut()
            .remove_resource::<ExtractResource<To, From>>()
        {
            app.get_sub_app_mut::<To>()
                .unwrap()
                .world_mut()
                .insert_resource(to);
        }
    }
}

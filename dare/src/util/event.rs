use bevy_ecs::prelude as becs;
use std::sync::mpsc::channel;

pub fn event_send<T: Send + 'static>() -> (EventSender<T>, EventReceiver<T>) {
    let (send, recv) = crossbeam_channel::unbounded();
    (EventSender::new(send), EventReceiver::new(recv))
}

#[derive(Debug, becs::Resource)]
pub struct EventSender<T: Send + 'static> {
    send: crossbeam_channel::Sender<T>,
}
impl<T: Send + 'static> Clone for EventSender<T> {
    fn clone(&self) -> Self {
        Self {
            send: self.send.clone(),
        }
    }
}
impl<T: Send + 'static> EventSender<T> {
    pub fn new(send: crossbeam_channel::Sender<T>) -> Self {
        Self { send }
    }

    pub fn send(&self, event: T) -> Result<(), crossbeam_channel::SendError<T>> {
        self.send.send(event)
    }
}

#[derive(Debug, becs::Resource)]
pub struct EventReceiver<T: Send + 'static> {
    recv: crossbeam_channel::Receiver<T>,
}
impl<T: Send + 'static> EventReceiver<T> {
    pub fn new(recv: crossbeam_channel::Receiver<T>) -> Self {
        Self { recv }
    }
}

impl<T: Send + 'static> Iterator for EventReceiver<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.recv.try_recv().ok()
    }
}

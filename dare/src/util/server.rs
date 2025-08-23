//! A dependency injection of a server architecture

use std::future::Future;
use std::sync::Arc;
use tokio::sync::mpsc::error::SendError;
/*
/// This is a basic struct used to let us bind callbacks
struct PacketHolder<Packet> {
    /// Packet in question
    packet: Packet,
    /// Callback to indicate packet has completed execution
    callback: Option<tokio::sync::oneshot::Sender<()>>,
}

/// The server component of the server/client model
///
/// # Server Context
/// The server context is a context that is exclusive with the server only
///
/// # Shared Context
/// A context that is shared between both the client and server
///
/// # Packet Type
pub struct Server<ServerContext, SharedContext, PacketType, PacketHandler, HandlerFuture> where
    PacketHandler: FnMut(Arc<SharedContext>, &mut ServerContext, PacketType) -> HandlerFuture + Send + 'static,
    HandlerFuture: Future<Output = ()> + Send + 'static,
    SharedContext: Send + 'static
{
}

impl<ServerContext, SharedContext, PacketType, PacketHandler, HandlerFuture> Server<ServerContext, SharedContext, PacketType, PacketHandler, HandlerFuture> where
    PacketHandler: FnMut(Arc<SharedContext>, &mut ServerContext, PacketType) -> HandlerFuture + Send + 'static,
    HandlerFuture: Future<Output = ()> + Send + 'static,
    SharedContext: Send + 'static {
    pub fn new(runtime: tokio::runtime::Handle, recv: tokio::sync::mpsc::UnboundedReceiver<PacketHolder<PacketType>>) -> Self {
        let rt = super::super::concurrent::tokio::BevyTokioRunTime::new(runtime);
        let thread = rt.runtime.spawn(async move {

            loop {
                if recv.is_closed() {
                    break;
                }
                match recv.recv()
            }
        });

        Self {

        }
    }
}

/// Client component of the server/client model
///
/// # Shared Context
/// A context that is shared between both the client and server and typically used to pass on important data
///
///
pub struct Client<SharedContext, Packet> where
    SharedContext: Send + 'static
{
    context: Arc<SharedContext>,
    send: tokio::sync::mpsc::UnboundedSender<PacketHolder<Packet>>,
}

impl<SharedContext, Packet> Client<SharedContext, Packet> where
    SharedContext: Send + 'static {

    /// Sends a non-blocking packet to the server
    pub fn send(&self, packet: Packet) -> Result<(), SendError<PacketHolder<Packet>>> {
        self.send.send(PacketHolder { packet, callback: None })
    }

    /// Sends a blocking packet to the server
    pub fn block_send(&self, packet: Packet) -> anyhow::Result<()> {
        let (callback_send, callback_recv) = tokio::sync::oneshot::channel();
        self.send.send(PacketHolder { packet, callback: Some(callback_send) })?;
        callback_recv.blocking_recv()?;
        Ok(())
    }
}
*/

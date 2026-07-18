use dagal::allocators::Allocator;
use dagal::ash::vk;

use super::{CoreContext, PresentContext, SwapchainContext};
use crate::transfer_belt::{TransferManager, TransferPool};

/// Owns all GPU objects for the render sub-app. Device `Arc` clones held by children
/// must be released before [`CoreContext`] drops — enforced by [`Self::shutdown`].
#[derive(Debug)]
pub struct RenderGpu<A: Allocator> {
    pub core: CoreContext,
    pub present: PresentContext,
    pub swapchain: SwapchainContext<A>,
    pub transfer: TransferManager<A>,
    pub transfer_pool: TransferPool<A>,
}

impl<A: Allocator> RenderGpu<A> {
    pub fn resize(&mut self, extent: vk::Extent2D) -> dagal::Result<()> {
        self.swapchain.resize(extent, &mut self.present, &self.core)
    }

    pub fn recreate(
        &mut self,
        extent: vk::Extent2D,
        handles: dare_window::WindowHandles,
    ) -> dagal::Result<()> {
        self.swapchain
            .recreate(extent, handles, &mut self.present, &self.core)
    }

    pub fn shutdown(self) {
        unsafe {
            let _ = self.core.device.get_handle().device_wait_idle();
        }

        let Self {
            core,
            present,
            swapchain,
            transfer,
            transfer_pool,
        } = self;

        drop(transfer_pool);
        drop(transfer);
        drop(present);
        drop(swapchain);
        drop(core);
    }
}

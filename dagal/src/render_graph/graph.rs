use ash::vk;

use super::*;
use crate::allocators::Allocator;
use crate::resource::traits::{Buildable, Resource};
use crate::resource::{Buffer, BufferDesc, Image, ImageDesc};
use crate::traits::AsRaw;
use std::collections::HashMap;
use std::fmt::Debug;

/// 1:many relationship where 1 writer pass feeds into multiple reader passes
#[derive(Default)]
struct ResourceState {
    /// [`None`] indicates a first access to a resource in the graph
    writer: Option<usize>,
    /// All readers of said resource produced by the writer
    readers: std::collections::HashSet<usize>,
}

#[derive(Default)]
pub struct RenderGraph<'a> {
    passes: Vec<Pass<'a>>,
    resources: VirtualResourceCollection<'a>,
    /// Current writer/readers for each resource, used to derive dependency edges
    resource_states: HashMap<UntypedNoGenerationVirtualResource, ResourceState>,
}

impl<'a> Debug for RenderGraph<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderGraph")
            .field("passes", &self.passes)
            .finish()
    }
}

impl<'a> RenderGraph<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create<A: Buildable + 'static>(&mut self, description: A::Desc) -> VirtualResource<A> {
        self.resources.create::<A>(description)
    }

    pub fn import<A: Resource + 'static>(&mut self, resource: &'a mut A) -> VirtualResource<A> {
        self.resources.import::<A>(resource)
    }

    /// Create a lazy initialized image
    pub fn create_image<A: Allocator + 'static>(
        &mut self,
        desc: ImageDesc,
    ) -> VirtualResource<Image<A>> {
        self.resources.create::<Image<A>>(desc)
    }

    /// Create a lazy initialized buffer
    pub fn create_buffer<A: Allocator + 'static>(
        &mut self,
        desc: BufferDesc,
    ) -> VirtualResource<Buffer<A>> {
        self.resources.create::<Buffer<A>>(desc)
    }

    pub fn import_image<A: Allocator + 'static>(
        &mut self,
        image: &'a mut Image<A>,
    ) -> VirtualResource<Image<A>> {
        self.resources.import::<Image<A>>(image)
    }

    pub fn import_buffer<A: Allocator + 'static>(
        &mut self,
        buffer: &'a mut Buffer<A>,
    ) -> VirtualResource<Buffer<A>> {
        self.resources.import::<Buffer<A>>(buffer)
    }

    pub fn add_pass(&mut self, configuration: PassConfiguration<'a>) -> PassBuilder<'_, 'a> {
        PassBuilder::new(self, configuration)
    }

    pub(crate) fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Adds pass to as a reader to a resource, returns which writer is to said resource
    pub(crate) fn record_read(
        &mut self,
        identity: UntypedNoGenerationVirtualResource,
        pass_index: usize,
    ) -> Option<usize> {
        let state = self.resource_states.entry(identity).or_default();
        let mut dependencies: Vec<usize> = state
            .writer
            .filter(|&writer| writer != pass_index)
            .into_iter()
            .collect();
        state.readers.insert(pass_index);
        dependencies.pop()
    }

    /// Indicate a pass writes to a given resource. Resets all readers to a resource, and sets pass
    /// as new writer. Returns a hash set of readers + writer - pass_index as dependencies
    pub(crate) fn record_write(
        &mut self,
        identity: UntypedNoGenerationVirtualResource,
        pass_index: usize,
    ) -> std::collections::HashSet<usize> {
        let state = self.resource_states.entry(identity).or_default();
        let mut dependencies: std::collections::HashSet<usize> = state
            .readers
            .iter()
            .copied()
            .filter(|&reader| reader != pass_index)
            .collect();
        if let Some(writer) = state.writer
            && writer != pass_index
        {
            dependencies.insert(writer);
        }
        state.readers.clear();
        state.writer = Some(pass_index);
        dependencies
    }

    pub(crate) fn push_pass(&mut self, pass: Pass<'a>) {
        self.passes.push(pass);
    }

    /// Execute the graph
    pub fn execute(self, queue: &crate::device::Queue) -> crate::Result<()> {
        let device = queue.device().clone();

        let pool =
            crate::command::CommandPool::new(crate::command::CommandPoolCreateInfo::WithQueue {
                device: device.clone(),
                queue,
                flags: vk::CommandPoolCreateFlags::empty(),
            })?;
        let recording = pool
            .allocate(1)?
            .pop()
            .unwrap()
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .unwrap();

        for pass in &self.passes {
            // match inputs with outputs
            // (In, Option<Out>)
            let mut resource: HashMap<
                UntypedVirtualResource,
                (ResourceAccess, Option<ResourceAccess>),
            > = pass
                .inputs
                .iter()
                .map(|usage| (usage.resource, (usage.access, None)))
                .collect();
            for out in pass.outputs.iter() {
                if let Some((_input, output)) = resource.get_mut(&out.resource) {
                    *output = Some(out.access);
                }
            }
            resource.retain(|_, (_, out)| out.is_some());

            // create proper resource transitions

            let mem = [vk::MemoryBarrier2::default()
                .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                .src_access_mask(vk::AccessFlags2::MEMORY_WRITE)
                .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                .dst_access_mask(vk::AccessFlags2::MEMORY_WRITE)];
            let dep = vk::DependencyInfo::default().memory_barriers(&mem);
            unsafe {
                device
                    .get_handle()
                    .cmd_pipeline_barrier2(*recording.as_raw(), &dep);
            }
            (pass.execution)(PassContext {
                device: device.clone(),
                command_buffer: &recording,
            })
            .unwrap();
        }

        let executable = recording.end().unwrap();
        let command_infos = [executable.submit_info()];
        let submits = [vk::SubmitInfo2::default().command_buffer_infos(&command_infos)];
        let fence = crate::sync::Fence::new(device.clone(), vk::FenceCreateFlags::empty())?;
        queue.submit2_and_wait_fence(&submits, &fence)?;

        Ok(())
    }
}

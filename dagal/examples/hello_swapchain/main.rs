use std::ptr;

use dagal::allocators::GPUAllocatorImpl;
use dagal::ash::vk;
use dagal::command::command_buffer::CmdBuffer;
use dagal::raw_window_handle::HasDisplayHandle;
use dagal::resource::traits::Resource;
use dagal::winit;
use dagal::wsi::WindowDimensions;

const FRAME_OVERLAP: usize = 2;

#[derive(Default)]
struct App {
    window: Option<winit::window::Window>,
    render_context: Option<RenderContext>,
}

struct RenderContext {
    draw_image_view: Option<dagal::resource::ImageView>,
    draw_image: Option<dagal::resource::Image<GPUAllocatorImpl>>,

    frames: Vec<Frame>,
    frame_number: usize,

    resize_requested: bool, // Whether frame needs to be resized
    swapchain_image_views: Vec<dagal::resource::ImageView>,
    swapchain_images: Vec<dagal::resource::Image<GPUAllocatorImpl>>,
    swapchain: Option<dagal::wsi::Swapchain>,
    surface: Option<dagal::wsi::Surface>,

    allocator: dagal::allocators::ArcAllocator<GPUAllocatorImpl>,
    graphics_queue: dagal::device::Queue,
    device: dagal::device::LogicalDevice,
    physical_device: dagal::device::PhysicalDevice,
    instance: dagal::core::Instance,
}

struct Frame {
    command_pool: dagal::command::CommandPool,
    command_buffer: dagal::command::CommandBuffer,

    swapchain_semaphore: dagal::sync::BinarySemaphore,
    render_semaphore: dagal::sync::BinarySemaphore,
    render_fence: dagal::sync::Fence,
}

impl RenderContext {
    fn new(rdh: dagal::raw_window_handle::RawDisplayHandle) -> Self {
        let mut instance = dagal::bootstrap::InstanceBuilder::new()
            .set_vulkan_version((1, 3, 0))
            .set_validation(true);
        for layer in dagal::ash_window::enumerate_required_extensions(rdh)
            .unwrap()
            .iter()
        {
            instance = instance.add_extension(*layer);
        }
        let instance = instance.build().unwrap();
        let debug_messenger =
            dagal::device::DebugMessenger::new(instance.get_entry(), instance.get_instance())
                .unwrap();

        let graphics_queue = dagal::bootstrap::QueueRequest::new(vk::QueueFlags::COMPUTE, 1, true);
        let physical_device = dagal::bootstrap::PhysicalDeviceSelector::default()
            .add_required_extension(dagal::ash::khr::swapchain::NAME.as_ptr())
            .set_minimum_vulkan_version((1, 3, 0))
            .add_required_queue(graphics_queue.clone())
            .select(&instance)
            .unwrap();
        let device = dagal::bootstrap::LogicalDeviceBuilder::from(physical_device.clone())
            .attach_feature_1_3(vk::PhysicalDeviceVulkan13Features {
                dynamic_rendering: vk::TRUE,
                synchronization2: vk::TRUE,
                ..Default::default()
            })
            .attach_feature_1_2(vk::PhysicalDeviceVulkan12Features {
                buffer_device_address: vk::TRUE,
                descriptor_indexing: vk::TRUE,
                ..Default::default()
            })
            .build(&instance)
            .unwrap();

        let allocator = GPUAllocatorImpl::new(gpu_allocator::vulkan::AllocatorCreateDesc {
            instance: instance.get_instance().clone(),
            device: device.get_handle().clone(),
            physical_device: physical_device.handle(),
            debug_settings: gpu_allocator::AllocatorDebugSettings {
                log_memory_information: false,
                log_leaks_on_shutdown: true,
                store_stack_traces: false,
                log_allocations: false,
                log_frees: false,
                log_stack_traces: false,
            },
            buffer_device_address: true,
            allocation_sizes: Default::default(),
        })
        .unwrap();
        let allocator = dagal::allocators::ArcAllocator::new(allocator);

        assert!(!graphics_queue.borrow().get_queues().is_empty());
        let graphics_queue = graphics_queue.borrow().get_queues()[0];
        let physical_device: dagal::device::PhysicalDevice = physical_device.into();

        let frames: Vec<Frame> = (0..FRAME_OVERLAP)
            .map(|_| {
                let command_pool = dagal::command::CommandPool::new(
                    device.clone(),
                    &graphics_queue,
                    vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                )
                .unwrap();

                let command_buffer = command_pool.allocate(1).unwrap().pop().unwrap();
                let swapchain_semaphore = dagal::sync::BinarySemaphore::new(
                    device.clone(),
                    vk::SemaphoreCreateFlags::empty(),
                )
                .unwrap();
                let render_semaphore = dagal::sync::BinarySemaphore::new(
                    device.clone(),
                    vk::SemaphoreCreateFlags::empty(),
                )
                .unwrap();
                let render_fence =
                    dagal::sync::Fence::new(device.clone(), vk::FenceCreateFlags::SIGNALED)
                        .unwrap();

                Frame {
                    command_pool,
                    command_buffer,
                    swapchain_semaphore,
                    render_semaphore,
                    render_fence,
                }
            })
            .collect();

        Self {
            instance,
            physical_device,
            device,
            graphics_queue,
            allocator,

            surface: None,
            swapchain: None,
            swapchain_images: vec![],
            swapchain_image_views: vec![],
            resize_requested: false,

            frame_number: 0,
            frames,

            draw_image: None,
            draw_image_view: None,
        }
    }

    /// Builds a surface
    fn build_surface(&mut self, window: &winit::window::Window) {
        let mut surface = dagal::wsi::Surface::new::<winit::window::Window>(
            self.instance.get_entry(),
            self.instance.get_instance(),
            window,
        )
        .unwrap();
        surface
            .query_details(self.physical_device.handle())
            .unwrap();
        self.surface = Some(surface);
    }

    /// Builds a swapchain
    fn build_swapchain(&mut self, window: &winit::window::Window) {
        let swapchain = dagal::bootstrap::SwapchainBuilder::new(self.surface.as_ref().unwrap())
            .push_queue(&self.graphics_queue)
            .request_present_mode(vk::PresentModeKHR::MAILBOX)
            .request_present_mode(vk::PresentModeKHR::FIFO)
            .request_color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
            .request_image_format(vk::Format::B8G8R8A8_UNORM)
            .set_extent(vk::Extent2D {
                width: window.width(),
                height: window.height(),
            })
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::TRANSFER_DST)
            .build(self.instance.get_instance(), self.device.clone())
            .unwrap();
        self.swapchain = Some(swapchain);
        // get images + image views
        self.swapchain_images = self.swapchain.as_ref().unwrap().get_images().unwrap();
        self.swapchain_image_views = self
            .swapchain
            .as_ref()
            .unwrap()
            .get_image_views(
                self.swapchain_images
                    .as_slice()
                    .iter()
                    .map(|image| image.handle())
                    .collect::<Vec<vk::Image>>()
                    .as_slice(),
            )
            .unwrap();
    }

    fn create_draw_image(&mut self) {
        let image = dagal::resource::Image::new(dagal::resource::ImageCreateInfo::NewAllocated {
            device: self.device.clone(),
            image_ci: vk::ImageCreateInfo {
                s_type: vk::StructureType::IMAGE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::ImageCreateFlags::empty(),
                image_type: vk::ImageType::TYPE_2D,
                format: vk::Format::R16G16B16A16_SFLOAT,
                extent: vk::Extent3D {
                    width: self.swapchain.as_ref().unwrap().extent().height,
                    height: self.swapchain.as_ref().unwrap().extent().width,
                    depth: 1,
                },
                mip_levels: 1,
                array_layers: 1,
                samples: vk::SampleCountFlags::TYPE_1,
                tiling: vk::ImageTiling::OPTIMAL,
                usage: vk::ImageUsageFlags::TRANSFER_SRC
                    | vk::ImageUsageFlags::TRANSFER_DST
                    | vk::ImageUsageFlags::COLOR_ATTACHMENT
                    | vk::ImageUsageFlags::STORAGE,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                queue_family_index_count: 1,
                p_queue_family_indices: &self.graphics_queue.get_family_index(),
                initial_layout: vk::ImageLayout::UNDEFINED,
                _marker: Default::default(),
            },
            allocator: &mut self.allocator,
            location: dagal::allocators::MemoryLocation::GpuOnly,
            name: Some("Draw Image"),
        })
        .unwrap();
        let image_view =
            dagal::resource::ImageView::new(dagal::resource::ImageViewCreateInfo::FromCreateInfo {
                create_info: vk::ImageViewCreateInfo {
                    s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageViewCreateFlags::empty(),
                    image: image.handle(),
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: image.format(),
                    components: Default::default(),
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        base_mip_level: 0,
                        level_count: 1,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    _marker: Default::default(),
                },
                device: self.device.clone(),
            })
            .unwrap();
        self.draw_image = Some(image);
        self.draw_image_view = Some(image_view);
    }

    /// Resize swapchain
    fn resize_swapchain(&mut self, window: &winit::window::Window) {
        self.resize_requested = false;
        // wait until fences are signaled
        {
            let fences: Vec<vk::Fence> = self
                .frames
                .iter()
                .map(|fence| fence.render_fence.handle())
                .collect();
            unsafe {
                self.device
                    .get_handle()
                    .wait_for_fences(fences.as_slice(), true, 1_000_000_000)
                    .unwrap_unchecked();
            }
        }
        self.swapchain = None;
        self.surface = None;
        self.swapchain_image_views.clear();
        self.build_surface(window);
        self.build_swapchain(window);
        self.create_draw_image();
    }

    // Draw into the background
    fn draw_background(
        device: &dagal::device::LogicalDevice,
        cmd: &dagal::command::CommandBufferRecording,
        draw_image: &dagal::resource::Image,
        frame_number: usize,
    ) {
        let flash = (frame_number as f64 / 120.0).sin().abs();
        let clear_value = vk::ClearColorValue {
            float32: [0.0, 0.0, flash as f32, 0.0],
        };
        let clear_range =
            dagal::resource::Image::image_subresource_range(vk::ImageAspectFlags::COLOR);
        unsafe {
            device.get_handle().cmd_clear_color_image(
                cmd.handle(),
                draw_image.handle(),
                vk::ImageLayout::GENERAL,
                &clear_value,
                &[clear_range],
            );
        }
    }

    // Deals with drawing
    fn draw(&mut self) {
        let swapchain_frame = self
            .frames
            .get_mut(self.frame_number % FRAME_OVERLAP)
            .unwrap();
        // wait
        unsafe {
            swapchain_frame
                .render_fence
                .wait(1000000000)
                .unwrap_unchecked();
            swapchain_frame.render_fence.reset().unwrap();
        }
        // check if we can even render
        if self.draw_image.is_none() || self.swapchain.is_none() || self.surface.is_none() {
            return;
        }

        let swapchain_frame = self.frames.get(self.frame_number % FRAME_OVERLAP).unwrap();
        // get swapchain image
        let swapchain_image_index = self
            .swapchain
            .as_ref()
            .unwrap()
            .next_image_index(1000000000, Some(&swapchain_frame.swapchain_semaphore), None)
            .unwrap();
        let swapchain_image = self
            .swapchain_images
            .get(swapchain_image_index as usize)
            .unwrap();

        // start command buffer
        let cmd = swapchain_frame.command_buffer.clone();
        cmd.reset(vk::CommandBufferResetFlags::empty()).unwrap();
        let cmd = cmd
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .unwrap();

        // if redraw was requested stop
        if self.resize_requested {
            return;
        }

        // transition
        self.draw_image.as_ref().unwrap().transition(
            &cmd,
            &self.graphics_queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::GENERAL,
        );
        Self::draw_background(
            &self.device,
            &cmd,
            self.draw_image.as_ref().unwrap(),
            self.frame_number,
        );
        self.draw_image.as_ref().unwrap().transition(
            &cmd,
            &self.graphics_queue,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        );
        swapchain_image.transition(
            &cmd,
            &self.graphics_queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        );
        swapchain_image.copy_from(&cmd, self.draw_image.as_ref().unwrap());
        swapchain_image.transition(
            &cmd,
            &self.graphics_queue,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            vk::ImageLayout::PRESENT_SRC_KHR,
        );
        let cmd = cmd.end().unwrap();

        let cmd_submit_info = dagal::command::CommandBufferExecutable::submit_info(cmd.handle());
        let submit_info = dagal::command::CommandBufferExecutable::submit_info_sync(
            &[cmd_submit_info],
            &[swapchain_frame
                .swapchain_semaphore
                .submit_info(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)],
            &[swapchain_frame
                .render_semaphore
                .submit_info(vk::PipelineStageFlags2::ALL_GRAPHICS)],
        );
        let _cmd = cmd
            .submit(
                self.graphics_queue.handle(),
                &[submit_info],
                swapchain_frame.render_fence.handle(),
            )
            .unwrap();
        let present_info = vk::PresentInfoKHR {
            s_type: vk::StructureType::PRESENT_INFO_KHR,
            p_next: ptr::null(),
            wait_semaphore_count: 1,
            p_wait_semaphores: swapchain_frame.render_semaphore.get_handle(),
            swapchain_count: 1,
            p_swapchains: self.swapchain.as_ref().unwrap().get_handle(),
            p_image_indices: &swapchain_image_index,
            p_results: ptr::null_mut(),
            _marker: Default::default(),
        };
        unsafe {
            match self
                .swapchain
                .as_ref()
                .unwrap()
                .get_ext()
                .queue_present(self.graphics_queue.handle(), &present_info)
            {
                Ok(_) => {}
                Err(error) => match error {
                    vk::Result::ERROR_OUT_OF_DATE_KHR => {
                        return;
                    }
                    _ => panic!("Error in queue present"),
                },
            }
        }
        self.frame_number += 1;
    }
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        unsafe {
            self.device.get_handle().device_wait_idle().unwrap();
        }
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        self.window = Some(
            event_loop
                .create_window(
                    winit::window::WindowAttributes::default()
                        .with_title("DARE")
                        .with_resizable(true),
                )
                .unwrap(),
        );
        if self.render_context.as_mut().is_none() {
            self.render_context = Some(RenderContext::new(
                self.window
                    .as_ref()
                    .unwrap()
                    .display_handle()
                    .unwrap()
                    .as_raw(),
            ))
        }
        if self.render_context.as_ref().unwrap().surface.is_none() {
            self.render_context
                .as_mut()
                .unwrap()
                .build_surface(self.window.as_mut().unwrap());
        }
        self.render_context
            .as_mut()
            .unwrap()
            .build_swapchain(self.window.as_ref().unwrap());
        self.render_context.as_mut().unwrap().create_draw_image();
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let window: &winit::window::Window = match self.window.as_ref() {
            None => return,
            Some(window) => window,
        };

        match event {
            winit::event::WindowEvent::CloseRequested => {
                // wait for device to finish
                if let Some(render_context) = self.render_context.take() {
                    drop(render_context);
                }
                event_loop.exit();
            }
            winit::event::WindowEvent::Resized(_) => {
                if let Some(render_context) = self.render_context.as_mut() {
                    // prevent 0,0
                    if window.width() != 0 && window.height() != 0 {
                        render_context.resize_swapchain(window);
                    }
                }
            }
            winit::event::WindowEvent::RedrawRequested => {
                if let Some(render_context) = self.render_context.as_mut() {
                    // do not draw if window size is impossibly small
                    if window.width() != 0 && window.height() != 0 {
                        render_context.draw();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &winit::event_loop::ActiveEventLoop) {
        let window: &winit::window::Window = match self.window.as_ref() {
            None => return,
            Some(window) => window,
        };
        window.request_redraw();
    }
}

fn main() {
    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}

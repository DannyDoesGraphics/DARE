mod primitives;
mod assets;
mod ray_tracing;

use std::{mem, path, ptr, slice};
use std::io::Write;
use std::sync::Arc;

use dagal::allocators::{Allocator, GPUAllocatorImpl};
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use dagal::ash::vk;
use dagal::util::free_list_allocator::Handle;
use dagal::command::command_buffer::CmdBuffer;
use dagal::descriptor::GPUResourceTable;
use dagal::pipelines::{Pipeline, PipelineBuilder};
use dagal::raw_window_handle::HasDisplayHandle;
use dagal::resource::traits::Resource;
use dagal::shader::ShaderCompiler;
use dagal::traits::Destructible;
use dagal::{resource, winit};
use dagal::descriptor::bindless::bindless::ResourceInput;
use dagal::util::immediate_submit::ImmediateSubmitContext;
use dagal::util::ImmediateSubmit;
use dagal::wsi::WindowDimensions;
use crate::primitives::{GeometrySurface, GPUMeshBuffer, MeshAsset, Vertex};

const FRAME_OVERLAP: usize = 2;

#[derive(Default)]
struct App<'a> {
    window: Option<winit::window::Window>,
    render_context: Option<RenderContext<'a>>,
}

struct RenderContext<'a> {
    instance: dagal::core::Instance,
    physical_device: dagal::device::PhysicalDevice,
    device: dagal::device::LogicalDevice,
    deletion_stack: dagal::util::DeletionStack<'a>,
    wsi_deletion_stack: dagal::util::DeletionStack<'a>,
    graphics_queue: dagal::device::Queue,
    allocator: dagal::allocators::SlotMapMemoryAllocator<GPUAllocatorImpl>,
    immediate_submit: ImmediateSubmit,

    surface: Option<dagal::wsi::Surface>,
    swapchain: Option<dagal::wsi::Swapchain>,
    swapchain_images: Vec<resource::Image<GPUAllocatorImpl>>,
    swapchain_image_views: Vec<resource::ImageView>,
    resize_requested: bool, // Whether frame needs to be resized

    frame_number: usize,
    frames: Vec<Frame<'a>>,

    gpu_resource_table: GPUResourceTable,

    depth_image: Option<resource::Image<GPUAllocatorImpl>>,
    depth_image_view: Option<resource::ImageView>,
    draw_image: Option<resource::Image<GPUAllocatorImpl>>,
    draw_image_view: Option<resource::ImageView>,
    draw_image_descriptors: Option<vk::DescriptorSet>,

    global_descriptor_pool: dagal::descriptor::DescriptorPool,
    draw_image_descriptor_set_layout: dagal::descriptor::DescriptorSetLayout,

    gradient_pipeline: dagal::pipelines::ComputePipeline,
    mesh_pipeline: dagal::pipelines::GraphicsPipeline,
    test_meshes: Vec<Arc<MeshAsset>>,

    sampler: Option<Handle<resource::Sampler>>,
    white_image: Option<AllocatedImage>,
    black_image: Option<AllocatedImage>,
    grey_image: Option<AllocatedImage>,
    error_checkerboard_image: Option<AllocatedImage>,
}

struct Frame<'a> {
    deletion_stack: dagal::util::DeletionStack<'a>,
    command_pool: dagal::command::CommandPool,
    command_buffer: dagal::command::CommandBuffer,

    swapchain_semaphore: dagal::sync::BinarySemaphore,
    render_semaphore: dagal::sync::BinarySemaphore,
    render_fence: dagal::sync::Fence,
}

#[derive(Debug, Clone)]
#[repr(C, align(16))]
struct PushConstants {
    data1: glam::Vec4,
    data2: glam::Vec4,
    data3: glam::Vec4,
    data4: glam::Vec4,
}

#[derive(Debug, Clone)]
struct AllocatedImage<A: Allocator = GPUAllocatorImpl> {
    pub image: Handle<resource::Image<A>>,
    pub image_view: Handle<resource::ImageView>,
    pub gpu_rt: GPUResourceTable<A>,
}

impl<A: Allocator> Destructible for AllocatedImage<A> {
    fn destroy(&mut self) {
        self.gpu_rt.free_image(self.image.clone()).unwrap();
        self.gpu_rt.free_image_view(self.image_view.clone()).unwrap();
    }
}

/// Whether to enable validation layers or not
const VALIDATION: bool = false;

impl<'a> RenderContext<'a> {
    fn new(rdh: dagal::raw_window_handle::RawDisplayHandle) -> Self {
        let mut deletion_stack = dagal::util::DeletionStack::new();
        let mut instance = dagal::bootstrap::InstanceBuilder::new()
            .set_vulkan_version((1, 3, 0))
            .add_extension(dagal::ash::ext::debug_utils::NAME.as_ptr())
            .set_validation(VALIDATION);
        for layer in dagal::ash_window::enumerate_required_extensions(rdh)
            .unwrap()
            .iter()
        {
            instance = instance.add_extension(*layer);
        }
        let instance = instance.build().unwrap();
        deletion_stack.push_resource(&instance);
        if VALIDATION {
            let mut debug_messenger =
                dagal::device::DebugMessenger::new(instance.get_entry(), instance.get_instance())
                    .unwrap();
            deletion_stack.push(move || {
                debug_messenger.destroy();
            });
        }

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
                descriptor_binding_partially_bound: vk::TRUE,
                descriptor_binding_update_unused_while_pending: vk::TRUE,
                descriptor_binding_sampled_image_update_after_bind: vk::TRUE,
                descriptor_binding_storage_image_update_after_bind: vk::TRUE,
                descriptor_binding_uniform_buffer_update_after_bind: vk::TRUE,
                shader_storage_buffer_array_non_uniform_indexing: vk::TRUE,
                shader_sampled_image_array_non_uniform_indexing: vk::TRUE,
                shader_storage_image_array_non_uniform_indexing: vk::TRUE,
                ..Default::default()
            })
            .attach_feature_1_0(vk::PhysicalDeviceFeatures {
                shader_int64: vk::TRUE,
                ..Default::default()
            })
            .debug_utils(true)
            .build(&instance)
            .unwrap();
        // clean up
        {
            let device = device.clone();
            deletion_stack.push(move || unsafe { device.get_handle().destroy_device(None) });
        }

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
        }).unwrap();
        deletion_stack.push_resource(&allocator);
        let mut allocator = dagal::allocators::SlotMapMemoryAllocator::new(allocator);

        assert!(!graphics_queue.borrow().get_queues().is_empty());
        let graphics_queue = graphics_queue.borrow().get_queues()[0];
        let physical_device: dagal::device::PhysicalDevice = physical_device.into();
        let immediate_submit = ImmediateSubmit::new(device.clone(), graphics_queue).unwrap();

        let frames: Vec<Frame<'a>> = (0..FRAME_OVERLAP)
            .map(|_| {
                let command_pool = dagal::command::CommandPool::new(
                    device.clone(),
                    &graphics_queue,
                    vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER,
                )
                .unwrap();
                deletion_stack.push_resource(&command_pool);

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
                {
                    let mut swapchain_semaphore = swapchain_semaphore.clone();
                    let mut render_semaphore = render_semaphore.clone();
                    let mut render_fence = render_fence.clone();
                    deletion_stack.push(move || {
                        swapchain_semaphore.destroy();
                        render_semaphore.destroy();
                        render_fence.destroy()
                    });
                }

                Frame {
                    deletion_stack: dagal::util::DeletionStack::new(),
                    command_pool,
                    command_buffer,
                    swapchain_semaphore,
                    render_semaphore,
                    render_fence,
                }
            })
            .collect();

        let gpu_resource_table = GPUResourceTable::new(device.clone(), &mut allocator).unwrap();

        let global_descriptor_pool = dagal::descriptor::DescriptorPool::new(
            device.clone(),
            10,
            &[dagal::descriptor::PoolSizeRatio {
                descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
                ratio: 1.0,
            }],
        )
        .unwrap();
        deletion_stack.push_resource(&global_descriptor_pool);

        let compiler = dagal::shader::ShaderCCompiler::new();
        let draw_image_set_layout = dagal::descriptor::DescriptorSetLayoutBuilder::default()
            .add_binding(0, vk::DescriptorType::STORAGE_IMAGE)
            .build(
                device.clone(),
                vk::ShaderStageFlags::COMPUTE,
                ptr::null(),
                vk::DescriptorSetLayoutCreateFlags::empty(),
            )
            .unwrap();
        deletion_stack.push_resource(&draw_image_set_layout);
        let gradient_pipeline_layout = dagal::pipelines::PipelineLayoutBuilder::default()
            .push_descriptor_sets(vec![draw_image_set_layout.handle()])
            .push_push_constant_struct::<PushConstants>(vk::ShaderStageFlags::COMPUTE)
            .build(device.clone(), vk::PipelineLayoutCreateFlags::empty())
            .unwrap();
        let gradient_pipeline = dagal::pipelines::ComputePipelineBuilder::default()
            .replace_layout(gradient_pipeline_layout)
            .replace_shader_from_source_file(
                device.clone(),
                &compiler,
                path::PathBuf::from("./dare/shaders/gradient.comp"),
                vk::ShaderStageFlags::COMPUTE,
            )
            .unwrap()
            .build(device.clone())
            .unwrap();
        deletion_stack.push_resource(&gradient_pipeline);

        let triangle_pipeline_layout = dagal::pipelines::PipelineLayoutBuilder::default()
            .push_descriptor_sets(vec![gpu_resource_table.get_descriptor_layout()])
            .push_push_constant_struct::<primitives::GPUDrawPushConstants>(
                vk::ShaderStageFlags::VERTEX,
            )
            .push_bindless_gpu_resource_table(&gpu_resource_table)
            .build(device.clone(), vk::PipelineLayoutCreateFlags::empty())
            .unwrap();
        let triangle_pipeline = dagal::pipelines::GraphicsPipelineBuilder::default()
            .clear()
            .replace_layout(triangle_pipeline_layout)
            .replace_shader_from_source_file(
                device.clone(),
                &compiler,
                std::path::PathBuf::from("./dare/shaders/colored_triangle_mesh.vert"),
                vk::ShaderStageFlags::VERTEX,
            )
            .unwrap()
            .replace_shader_from_source_file(
                device.clone(),
                &compiler,
                std::path::PathBuf::from("./dare/shaders/tex_image.frag"),
                vk::ShaderStageFlags::FRAGMENT,
            )
            .unwrap()
            .set_input_topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .set_polygon_mode(vk::PolygonMode::FILL)
            .set_cull_mode(vk::CullModeFlags::NONE, vk::FrontFace::CLOCKWISE)
            .set_multisampling_none()
            .enable_blending_alpha_blend()
            .enable_depth_test(vk::TRUE, vk::CompareOp::GREATER_OR_EQUAL)
            .set_color_attachment(vk::Format::R16G16B16A16_SFLOAT)
            .set_depth_format(vk::Format::D32_SFLOAT)
            .build(device.clone())
            .unwrap();
        deletion_stack.push_resource(&triangle_pipeline);

        let mut app = Self {
            instance,
            physical_device,
            device,
            graphics_queue,
            deletion_stack,
            wsi_deletion_stack: dagal::util::DeletionStack::new(),
            allocator,
            immediate_submit,

            surface: None,
            swapchain: None,
            swapchain_images: vec![],
            swapchain_image_views: vec![],
            resize_requested: false,

            frame_number: 0,
            frames,

            gpu_resource_table,
            depth_image: None,
            depth_image_view: None,
            draw_image: None,
            draw_image_view: None,
            draw_image_descriptors: None,

            global_descriptor_pool,
            draw_image_descriptor_set_layout: draw_image_set_layout,

            gradient_pipeline,

            mesh_pipeline: triangle_pipeline,
            test_meshes: vec![],

            sampler: None,
            white_image: None,
            black_image: None,
            grey_image: None,
            error_checkerboard_image: None,
        };
        // create default images
        // create default texture data
        app.sampler = Some(app.gpu_resource_table.new_sampler(ResourceInput::ResourceCI(
            resource::SamplerCreateInfo::FromCreateInfo {
                device: app.device.clone(),
                create_info: vk::SamplerCreateInfo::default()
                    .mag_filter(vk::Filter::NEAREST)
                    .min_filter(vk::Filter::NEAREST),
                name: None,
            }
        )).unwrap());
        app.deletion_stack.push({
            let sampler = app.sampler.clone().unwrap();
            let mut gpu_rt = app.gpu_resource_table.clone();
            move || {
                gpu_rt.free_sampler(sampler).unwrap();
            }
        });

        let white = [255u8, 255u8, 255u8, 255u8];
        app.white_image = Some(app.create_image_with_data(white.as_slice(), vk::Extent3D {
            width: 1,
            height: 1,
            depth: 1,
        }, vk::Format::R8G8B8A8_UNORM, vk::ImageUsageFlags::SAMPLED,  Some(String::from("White")),false));
        let grey = [168u8, 168u8, 168u8, 255u8];
        app.grey_image = Some(app.create_image_with_data(grey.as_slice(), vk::Extent3D {
            width: 1,
            height: 1,
            depth: 1,
        }, vk::Format::R8G8B8A8_UNORM, vk::ImageUsageFlags::SAMPLED,  Some(String::from("Gray")),false));
        let black = [0u8, 0u8, 0u8, 255u8];
        app.black_image = Some(app.create_image_with_data(black.as_slice(), vk::Extent3D {
            width: 1,
            height: 1,
            depth: 1,
        }, vk::Format::R8G8B8A8_UNORM, vk::ImageUsageFlags::SAMPLED,  Some(String::from("Black")),false));
        let mut pixels = [64u8 ; 16 * 16 * 4];
        let magenta = [255u8, 0u8, 255u8, 255u8];
        for x in 0..16 {
            for y in 0..16 {
                let index = (y*16 + x)*4;
                if (x % 2) ^ (y % 2) != 0 {
                    pixels[index..index+4].copy_from_slice(&magenta);
                } else {
                    pixels[index..index+4].copy_from_slice(&black);
                }
            }
        }
        app.error_checkerboard_image = Some(app.create_image_with_data(pixels.as_slice(), vk::Extent3D {
            width: 16,
            height: 16,
            depth: 1,
        }, vk::Format::R8G8B8A8_UNORM, vk::ImageUsageFlags::SAMPLED,  Some(String::from("Magenta")),false));

        app.deletion_stack.push_resource(app.white_image.as_ref().unwrap());
        app.deletion_stack.push_resource(app.grey_image.as_ref().unwrap());
        app.deletion_stack.push_resource(app.black_image.as_ref().unwrap());
        app.deletion_stack.push_resource(app.error_checkerboard_image.as_ref().unwrap());

        let meshes = app.load_gltf_meshes(std::path::PathBuf::from("./dare/assets/basicmesh.glb"));
        app.test_meshes = meshes;
        app
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
        self.wsi_deletion_stack.push_resource(&surface);
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
        self.wsi_deletion_stack.push_resource(&swapchain);
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
        self.wsi_deletion_stack
            .push_resources(self.swapchain_image_views.as_slice());
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
                    width: self.swapchain.as_ref().unwrap().extent().width,
                    height: self.swapchain.as_ref().unwrap().extent().height,
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
            name: Some(String::from("Draw image")),
        })
        .unwrap();
        //self.wsi_deletion_stack.push_resource(&image);
        let depth_image = dagal::resource::Image::new(dagal::resource::ImageCreateInfo::NewAllocated {
            device: self.device.clone(),
            image_ci: vk::ImageCreateInfo {
                s_type: vk::StructureType::IMAGE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::ImageCreateFlags::empty(),
                image_type: vk::ImageType::TYPE_2D,
                format: vk::Format::D32_SFLOAT,
                extent: vk::Extent3D {
                    width: self.swapchain.as_ref().unwrap().extent().width,
                    height: self.swapchain.as_ref().unwrap().extent().height,
                    depth: 1,
                },
                mip_levels: 1,
                array_layers: 1,
                samples: vk::SampleCountFlags::TYPE_1,
                tiling: vk::ImageTiling::OPTIMAL,
                usage: vk::ImageUsageFlags::DEPTH_STENCIL_ATTACHMENT,
                sharing_mode: vk::SharingMode::EXCLUSIVE,
                queue_family_index_count: 1,
                p_queue_family_indices: &self.graphics_queue.get_family_index(),
                initial_layout: vk::ImageLayout::UNDEFINED,
                _marker: Default::default(),
            },
            allocator: &mut self.allocator,
            location: dagal::allocators::MemoryLocation::GpuOnly,
            name: Some(String::from("GBuffer Depth")),
        }).unwrap();
        //self.wsi_deletion_stack.push_resource(&depth_image);
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
                    subresource_range: dagal::resource::Image::image_subresource_range(vk::ImageAspectFlags::COLOR),
                    _marker: Default::default(),
                },
                device: self.device.clone(),
            })
            .unwrap();
        let depth_image_view =
            dagal::resource::ImageView::new(dagal::resource::ImageViewCreateInfo::FromCreateInfo {
                create_info: vk::ImageViewCreateInfo {
                    s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageViewCreateFlags::empty(),
                    image: depth_image.handle(),
                    view_type: vk::ImageViewType::TYPE_2D,
                    format: depth_image.format(),
                    components: Default::default(),
                    subresource_range: dagal::resource::Image::image_subresource_range(vk::ImageAspectFlags::DEPTH),
                    _marker: Default::default(),
                },
                device: self.device.clone(),
            })
                .unwrap();
        self.wsi_deletion_stack.push_resource(&image_view);
        self.wsi_deletion_stack.push_resource(&depth_image_view);
        self.draw_image = Some(image);
        self.depth_image = Some(depth_image);
        self.draw_image_view = Some(image_view);
        self.depth_image_view = Some(depth_image_view);

        // update descriptors
        self.global_descriptor_pool
            .reset(vk::DescriptorPoolResetFlags::empty())
            .unwrap();
        self.draw_image_descriptors = Some(
            self.global_descriptor_pool
                .allocate(self.draw_image_descriptor_set_layout.handle())
                .unwrap(),
        );
        let img_info = vk::DescriptorImageInfo {
            sampler: Default::default(),
            image_view: self.draw_image_view.as_ref().unwrap().handle(),
            image_layout: vk::ImageLayout::GENERAL,
        };
        let write_descriptor_set = vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            p_next: ptr::null(),
            dst_set: self.draw_image_descriptors.unwrap(),
            dst_binding: 0,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::STORAGE_IMAGE,
            p_image_info: &img_info,
            p_buffer_info: ptr::null(),
            p_texel_buffer_view: ptr::null(),
            _marker: Default::default(),
        };
        unsafe {
            self.device
                .get_handle()
                .update_descriptor_sets(&[write_descriptor_set], &[]);
        }
    }

    /// Resize swapchain
    fn resize_swapchain(&mut self, window: &winit::window::Window) {
        println!(
            "Resize requested with extents: {} x {}",
            window.width(),
            window.height()
        );
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
        if let Some(mut draw_image) = self.draw_image.take() {
            draw_image.destroy();
        }
        if let Some(mut depth_image) = self.depth_image.take() {
            depth_image.destroy();
        }
        //self.depth_image = None;
        //self.draw_image = None;
        self.wsi_deletion_stack.flush();
        self.swapchain = None;
        self.surface = None;
        self.swapchain_image_views.clear();
        self.build_surface(window);
        self.build_swapchain(window);
        self.create_draw_image();
        self.resize_requested = false;
    }

    // Draw into the background
    fn draw_background(
        device: &dagal::device::LogicalDevice,
        cmd: &dagal::command::CommandBufferRecording,
        draw_image: &dagal::resource::Image,
        frame_number: usize,
        mut gradient_pipeline: dagal::pipelines::ComputePipeline,
        gradient_descriptor_set: vk::DescriptorSet,
    ) {
        let flash = (frame_number as f64 / 120.0).sin().abs();
        let clear_value = vk::ClearColorValue {
            float32: [0.0, 0.0, flash as f32, 0.0],
        };
        let clear_range =
            dagal::resource::Image::image_subresource_range(vk::ImageAspectFlags::COLOR);
        unsafe {
            device.get_handle().cmd_bind_pipeline(
                cmd.handle(),
                vk::PipelineBindPoint::COMPUTE,
                gradient_pipeline.handle(),
            );
            device.get_handle().cmd_bind_descriptor_sets(
                cmd.handle(),
                vk::PipelineBindPoint::COMPUTE,
                gradient_pipeline.layout(),
                0,
                &[gradient_descriptor_set],
                &[],
            );
            let pc = PushConstants {
                data1: glam::Vec4::new(
                    (((frame_number as f64 % f32::MAX as f64) / 240.0)
                        .sin()
                        .abs() as f32)
                        + 1.0,
                    0.0,
                    0.0,
                    1.0,
                ),
                data2: glam::Vec4::new(
                    0.0,
                    0.0,
                    (((frame_number as f64 % f32::MAX as f64) / 240.0)
                        .cos()
                        .abs() as f32)
                        + 1.0,
                    1.0,
                ),
                data3: glam::Vec4::splat(0.0),
                data4: glam::Vec4::splat(0.0),
            };
            device.get_handle().cmd_push_constants(
                cmd.handle(),
                gradient_pipeline.layout(),
                vk::ShaderStageFlags::COMPUTE,
                0,
                unsafe {
                    slice::from_raw_parts(
                        &pc as *const PushConstants as *const u8,
                        mem::size_of::<PushConstants>(),
                    )
                },
            );
            device.get_handle().cmd_dispatch(
                cmd.handle(),
                (draw_image.extent().width as f32 / 16.0).ceil() as u32,
                (draw_image.extent().height as f32 / 16.0).ceil() as u32,
                1,
            )
        }
    }

    fn draw_geometry(
        device: &dagal::device::LogicalDevice,
        cmd: &dagal::command::CommandBufferRecording,
        draw_image: &dagal::resource::Image,
        draw_image_view: &dagal::resource::ImageView,
        depth_image_view: &dagal::resource::ImageView,
        mut mesh_pipeline: dagal::pipelines::GraphicsPipeline,
        gpu_rt: &GPUResourceTable,
        meshes: Vec<Arc<MeshAsset>>,
    ) {
        let dynamic_rendering_context = cmd.dynamic_rendering();
        let dynamic_rendering_context = dynamic_rendering_context
            .push_image_as_color_attachment(vk::ImageLayout::GENERAL, draw_image_view, None)
            .depth_attachment_info(depth_image_view.handle(), vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
            .begin_rendering(vk::Extent2D {
                width: draw_image.extent().width,
                height: draw_image.extent().height,
            });
        unsafe {
            device.get_handle().cmd_bind_pipeline(
                cmd.handle(),
                vk::PipelineBindPoint::GRAPHICS,
                mesh_pipeline.handle(),
            );
            device.get_handle().cmd_bind_descriptor_sets(
                cmd.handle(),
                vk::PipelineBindPoint::GRAPHICS,
                mesh_pipeline.layout(),
                0,
                &[gpu_rt.get_descriptor_set()],
                &[],
            );
            let viewport = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: draw_image.extent().width as f32,
                height: draw_image.extent().height as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            device
                .get_handle()
                .cmd_set_viewport(cmd.handle(), 0, &[viewport]);
            let scissor = vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent: vk::Extent2D {
                    width: draw_image.extent().width,
                    height: draw_image.extent().height,
                },
            };
            device
                .get_handle()
                .cmd_set_scissor(cmd.handle(), 0, &[scissor]);
            let view = glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.0, -2.5));
            let mut projection = glam::Mat4::perspective_rh(70_f32.to_radians(),  draw_image.extent().width as f32 / draw_image.extent().height as f32, 10000.0, 0.1);
            projection.y_axis.y *= -1.0;
            let world_matrix = projection * view;
            let mesh_render = meshes.get(2).unwrap();
            let push_constants = primitives::GPUDrawPushConstants {
                world_matrix,
                vertex_buffer_id: mesh_render.mesh_buffers.vertex_buffer.id() as u32,
            };
            device.get_handle().cmd_push_constants(
                cmd.handle(),
                mesh_pipeline.layout(),
                vk::ShaderStageFlags::VERTEX,
                0,
                unsafe {
                    slice::from_raw_parts(
                        &push_constants as *const primitives::GPUDrawPushConstants as *const u8,
                        mem::size_of::<primitives::GPUDrawPushConstants>(),
                    )
                },
            );
            device.get_handle().cmd_bind_index_buffer(
                cmd.handle(),
                mesh_render.mesh_buffers.index_buffer.handle(),
                0,
                vk::IndexType::UINT32,
            );
            device
                .get_handle()
                .cmd_draw_indexed(cmd.handle(), mesh_render.surfaces[0].count, 1, mesh_render.surfaces[0].start_index, 0, 0);
            dynamic_rendering_context.end_rendering();
        }
    }

    fn load_gltf_meshes(&mut self, path: path::PathBuf) -> Vec<Arc<MeshAsset>> {
        let (document, buffers, images) = gltf::import(path).unwrap();
        let mut meshes: Vec<Arc<MeshAsset>> = Vec::new();
        let mut immediate_submit = dagal::util::ImmediateSubmit::new(self.device.clone(), self.graphics_queue).unwrap();

        let mut indices: Vec<u32> = Vec::new();
        let mut vertices: Vec<Vertex> = Vec::new();
        for mesh in document.meshes() {
            let mut surfaces: Vec<GeometrySurface> = Vec::new();
            vertices.clear();
            indices.clear();

            for primitive in mesh.primitives() {
                let start_index = indices.len() as u32;
                let count = primitive.indices().unwrap().count() as u32;
                let initial_vtx = vertices.len() as u32;

                let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                // load indices
                {
                    let idxes = reader.read_indices().unwrap();
                    let idxes = idxes.into_u32();
                    for index in idxes {
                        indices.push(index + initial_vtx);
                    }
                }

                // load vertices
                {
                    let pos = reader.read_positions().unwrap();
                    vertices.resize(vertices.len() + pos.clone().count(), Default::default());
                    for (index, pos) in pos.enumerate() {
                        *vertices.get_mut(initial_vtx as usize + index).unwrap() = Vertex {
                            position: glam::Vec3::from(pos),
                            uv_x: 0.0,
                            normal: glam::Vec3::new(1.0, 0.0, 0.0),
                            uv_y: 0.0,
                            color: glam::Vec4::ONE,
                        };
                    }
                }

                // load normals
                if let Some(normals) = reader.read_normals() {
                    for (index, normal) in normals.enumerate() {
                        vertices.get_mut(initial_vtx as usize + index).unwrap().normal = glam::Vec3::from(normal);
                    }
                }

                // load UVs
                if let Some(uvs) = reader.read_tex_coords(0) {
                    for (index, uv) in uvs.into_f32().enumerate() {
                        vertices.get_mut(initial_vtx as usize + index).unwrap().uv_x = uv[0];
                        vertices.get_mut(initial_vtx as usize + index).unwrap().uv_y = uv[1];
                    }
                }

                // load colors
                if let Some(colors) = reader.read_colors(0) {
                    for (index, color) in colors.into_rgba_f32().enumerate() {
                        vertices.get_mut(initial_vtx as usize + index).unwrap().color = glam::Vec4::from(color);
                    }
                }
                
                surfaces.push(GeometrySurface {
                    start_index,
                    count,
                })
            }
            
            // display normals instead
            {
                for vertex in vertices.iter_mut() {
                    vertex.color = glam::Vec4::from((vertex.normal, 1.0));
                }
            }
            let mesh_buffers = GPUMeshBuffer::new(&mut self.allocator, &mut immediate_submit, &mut self.gpu_resource_table, indices.as_slice(), vertices.as_slice(), Some(mesh.name().unwrap().to_string()));
            self.deletion_stack.push({
                let mut gpu_rt = self.gpu_resource_table.clone();
                let mut index_buffer = mesh_buffers.index_buffer.clone();
                let typed_buffer = mesh_buffers.vertex_buffer.clone();
                move || {
                    index_buffer.destroy();
                    gpu_rt.free_typed_buffer(typed_buffer).unwrap()
                }
            });
            meshes.push(Arc::new(MeshAsset {
                name: mesh.name().unwrap().to_string(),
                surfaces,
                mesh_buffers,
            }));
        }
        immediate_submit.destroy();
        meshes
    }

    fn create_image(&mut self, size: vk::Extent3D, format: vk::Format, usage: vk::ImageUsageFlags, name: Option<String>, mipmappings: bool) -> AllocatedImage {
        let mip_levels = if mipmappings {
            size.width.max(size.height).ilog2() + 1
        } else {
            1
        };
        let image = resource::Image::new(
            resource::ImageCreateInfo::NewAllocated {
                device: self.device.clone(),
                allocator: &mut self.allocator,
                location: dagal::allocators::MemoryLocation::GpuOnly,
                image_ci: vk::ImageCreateInfo {
                    s_type: vk::StructureType::IMAGE_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageCreateFlags::empty(),
                    image_type: vk::ImageType::TYPE_2D,
                    format,
                    extent: size,
                    mip_levels,
                    array_layers: 1,
                    samples: vk::SampleCountFlags::TYPE_1,
                    tiling: vk::ImageTiling::LINEAR,
                    usage,
                    sharing_mode: vk::SharingMode::EXCLUSIVE,
                    queue_family_index_count: 1,
                    p_queue_family_indices: &self.graphics_queue.get_family_index(),
                    initial_layout: vk::ImageLayout::UNDEFINED,
                    _marker: Default::default(),
                },
                name,
            }
        ).unwrap();
        let aspect_flag = if format == vk::Format::D32_SFLOAT {
            vk::ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };
        let mut subresource_range = resource::Image::image_subresource_range(aspect_flag);
        subresource_range.level_count = mip_levels;
        let image_view = resource::ImageView::new(
            resource::ImageViewCreateInfo::FromCreateInfo {
                device: self.device.clone(),
                create_info: vk::ImageViewCreateInfo {
                    s_type: vk::StructureType::IMAGE_VIEW_CREATE_INFO,
                    p_next: ptr::null(),
                    flags: vk::ImageViewCreateFlags::empty(),
                    image: image.handle(),
                    view_type: vk::ImageViewType::TYPE_2D,
                    format,
                    components: Default::default(),
                    subresource_range,
                    _marker: Default::default(),
                },
            }
        ).unwrap();
        let (image, image_view) = self.gpu_resource_table.new_image(ResourceInput::Resource(image), ResourceInput::Resource(image_view), vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL).unwrap();
        AllocatedImage {
            image,
            image_view,
            gpu_rt: self.gpu_resource_table.clone(),
        }
    }

    fn create_image_with_data<T: Sized>(&mut self, data: &[T] ,size: vk::Extent3D, format: vk::Format, usage: vk::ImageUsageFlags, name: Option<String>, mipmappings: bool) -> AllocatedImage {
        let data_size = size.width as u64 * size.height as u64 * size.depth as u64 * 4;
        let mut staging_buffer = resource::Buffer::new(
            resource::BufferCreateInfo::NewEmptyBuffer {
                device: self.device.clone(),
                allocator: &mut self.allocator,
                size: data_size,
                memory_type: dagal::allocators::MemoryLocation::CpuToGpu,
                usage_flags: vk::BufferUsageFlags::TRANSFER_SRC,
            }
        ).unwrap();
        staging_buffer.write(0, data).unwrap();
        // min expected flags
        let usage = usage | vk::ImageUsageFlags::TRANSFER_DST | vk::ImageUsageFlags::TRANSFER_SRC;
        let allocated_image = self.create_image(size, format, usage, name, mipmappings);
        self.immediate_submit.submit({
            let allocated_image = self.gpu_resource_table.get_image(&allocated_image.image).unwrap();
            let staging_buffer = staging_buffer.clone();
            move |context: ImmediateSubmitContext| {
                allocated_image.transition(context.cmd, context.queue, vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL);
                let copy_region = vk::BufferImageCopy {
                    buffer_offset: 0,
                    buffer_row_length: 0,
                    buffer_image_height: 0,
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    image_offset: Default::default(),
                    image_extent: allocated_image.extent(),
                };

                unsafe {
                    context.device.get_handle().cmd_copy_buffer_to_image(context.cmd.handle(), staging_buffer.handle(), allocated_image.handle(), vk::ImageLayout::TRANSFER_DST_OPTIMAL, &[copy_region]);
                }
                allocated_image.transition(context.cmd, context.queue, vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL)
            }
        });
        staging_buffer.destroy();
        allocated_image
    }

    // Deals with drawing
    fn draw(&mut self) {
        // clear out last frame
        self.frames
            .get_mut(self.frame_number % FRAME_OVERLAP)
            .unwrap()
            .deletion_stack
            .flush();
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
            swapchain_frame.deletion_stack.flush();
            swapchain_frame.render_fence.reset().unwrap();
        }
        // check if we can even render
        if self.draw_image.is_none() || self.swapchain.is_none() || self.surface.is_none() || self.depth_image.is_none() {
            return;
        }

        let swapchain_frame = self.frames.get(self.frame_number % FRAME_OVERLAP).unwrap();
        // get swapchain image
        let swapchain_image_index = self
            .swapchain
            .as_ref()
            .unwrap()
            .next_image_index(
                1000000000,
                Some(swapchain_frame.swapchain_semaphore.clone()),
                None,
            );
        if swapchain_image_index.is_err() {
            return;
        }
        let swapchain_image_index = swapchain_image_index.unwrap();
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
            self.gradient_pipeline.clone(),
            self.draw_image_descriptors.unwrap(),
        );
        // add a sync point
        self.draw_image.as_ref().unwrap().transition(
            &cmd,
            &self.graphics_queue,
            vk::ImageLayout::GENERAL,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        );
        self.depth_image.as_ref().unwrap().transition(
            &cmd,
            &self.graphics_queue,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
        );
        Self::draw_geometry(
            &self.device,
            &cmd,
            self.draw_image.as_ref().unwrap(),
            self.draw_image_view.as_ref().unwrap(),
            self.depth_image_view.as_ref().unwrap(),
            self.mesh_pipeline.clone(),
            &self.gpu_resource_table,
            self.test_meshes.clone(),
        );
        self.draw_image.as_ref().unwrap().transition(
            &cmd,
            &self.graphics_queue,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
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
        let cmd = cmd
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

impl<'a> Drop for RenderContext<'a> {
    fn drop(&mut self) {
        unsafe {
            self.device.get_handle().device_wait_idle().unwrap();
        }
        self.wsi_deletion_stack.flush();
        if let Some(mut draw_image) = self.draw_image.take() {
            draw_image.destroy();
        }
        if let Some(mut depth_image) = self.depth_image.take() {
            depth_image.destroy();
        }
        self.gpu_resource_table.destroy();
        self.immediate_submit.destroy();
        self.deletion_stack.flush();
    }
}

impl<'a> winit::application::ApplicationHandler for App<'a> {
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
                        render_context.resize_requested = true;
                        render_context.resize_swapchain(window);
                    }
                }
            }
            winit::event::WindowEvent::RedrawRequested => {
                if let Some(render_context) = self.render_context.as_mut() {
                    // do not draw if window size is impossibly small
                    if window.width() != 0 && window.height() != 0 && !render_context.resize_requested {
                        render_context.draw();
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        let window: &winit::window::Window = match self.window.as_ref() {
            None => return,
            Some(window) => window,
        };
        window.request_redraw();
    }
}

fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}

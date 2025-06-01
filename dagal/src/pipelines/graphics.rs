use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::fmt::Debug;
use std::ptr;

use ash::vk;

use crate::traits::Destructible;

#[derive(Debug)]
pub struct GraphicsPipeline {
    handle: vk::Pipeline,
    device: crate::device::LogicalDevice,
}

impl Destructible for GraphicsPipeline {
    fn destroy(&mut self) {
        #[cfg(feature = "log-lifetimes")]
        tracing::trace!("Destroying VkPipeline {:p}", self.handle);

        unsafe {
            self.device.get_handle().destroy_pipeline(self.handle, None);
        }
    }
}

#[cfg(feature = "raii")]
impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        self.destroy();
    }
}

impl super::Pipeline for GraphicsPipeline {
    fn handle(&self) -> vk::Pipeline {
        self.handle
    }

    fn get_device(&self) -> &crate::device::LogicalDevice {
        &self.device
    }
}

#[derive(Debug)]
pub struct GraphicsPipelineBuilder<'a> {
    shaders: HashMap<vk::ShaderStageFlags, crate::shader::Shader>,

    input_assembly: vk::PipelineInputAssemblyStateCreateInfo<'a>,
    rasterizer: vk::PipelineRasterizationStateCreateInfo<'a>,
    color_blend_attachment: vk::PipelineColorBlendAttachmentState,
    multisampling: vk::PipelineMultisampleStateCreateInfo<'a>,
    layout: Option<vk::PipelineLayout>,
    depth_stencil: vk::PipelineDepthStencilStateCreateInfo<'a>,
    render_info: vk::PipelineRenderingCreateInfo<'a>,
    color_attachment_format: vk::Format,
}

impl Clone for GraphicsPipelineBuilder<'_> {
    /// **Only performs a partial clone of the underlying data.**
    ///
    /// Only clones input_assembly, rasterizer, color_blend_attachment, multisampling, depth_stencil,
    /// render_info, and color_attachment_info.
    ///
    /// In other words, **does not clone layout or shaders**.
    fn clone(&self) -> Self {
        Self {
            shaders: Default::default(),
            input_assembly: self.input_assembly,
            rasterizer: self.rasterizer,
            color_blend_attachment: self.color_blend_attachment,
            multisampling: self.multisampling,
            layout: self.layout,
            depth_stencil: self.depth_stencil,
            render_info: self.render_info,
            color_attachment_format: self.color_attachment_format,
        }
    }
}

impl Default for GraphicsPipelineBuilder<'_> {
    fn default() -> Self {
        Self {
            shaders: HashMap::new(),
            input_assembly: vk::PipelineInputAssemblyStateCreateInfo {
                s_type: vk::StructureType::PIPELINE_INPUT_ASSEMBLY_STATE_CREATE_INFO,
                p_next: ptr::null(),
                ..Default::default()
            },
            rasterizer: vk::PipelineRasterizationStateCreateInfo {
                s_type: vk::StructureType::PIPELINE_RASTERIZATION_STATE_CREATE_INFO,
                p_next: ptr::null(),
                ..Default::default()
            },
            color_blend_attachment: vk::PipelineColorBlendAttachmentState::default(),
            multisampling: vk::PipelineMultisampleStateCreateInfo {
                s_type: vk::StructureType::PIPELINE_MULTISAMPLE_STATE_CREATE_INFO,
                p_next: ptr::null(),
                ..Default::default()
            },
            layout: None,
            depth_stencil: vk::PipelineDepthStencilStateCreateInfo {
                s_type: vk::StructureType::PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
                p_next: ptr::null(),
                ..Default::default()
            },
            render_info: vk::PipelineRenderingCreateInfo {
                s_type: vk::StructureType::PIPELINE_RENDERING_CREATE_INFO,
                p_next: ptr::null(),
                ..Default::default()
            },
            color_attachment_format: Default::default(),
        }
    }
}

impl super::PipelineBuilder for GraphicsPipelineBuilder<'_> {
    type BuildTo = GraphicsPipeline;

    fn replace_layout(mut self, layout: vk::PipelineLayout) -> Self {
        self.layout = Some(layout);
        self
    }

    fn replace_shader(
        mut self,
        shader: crate::shader::Shader,
        stage: vk::ShaderStageFlags,
    ) -> Self {
        if let Some(shader) = self.shaders.remove(&stage) {
            drop(shader);
        }
        self.shaders.insert(stage, shader);
        self
    }

    /// Builds the compute pipeline
    fn build(mut self, device: crate::device::LogicalDevice) -> anyhow::Result<Self::BuildTo> {
        let viewport_state = vk::PipelineViewportStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_VIEWPORT_STATE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::PipelineViewportStateCreateFlags::empty(),
            viewport_count: 1,
            p_viewports: ptr::null(),
            scissor_count: 1,
            p_scissors: ptr::null(),
            _marker: Default::default(),
        };

        let color_blending = vk::PipelineColorBlendStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_COLOR_BLEND_STATE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::PipelineColorBlendStateCreateFlags::empty(),
            logic_op_enable: vk::FALSE,
            logic_op: vk::LogicOp::COPY,
            attachment_count: 1,
            p_attachments: &self.color_blend_attachment,
            blend_constants: [0.0, 0.0, 0.0, 0.0],
            _marker: Default::default(),
        };

        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_VERTEX_INPUT_STATE_CREATE_INFO,
            p_next: ptr::null(),
            ..Default::default()
        };

        let dynamic_states: Vec<vk::DynamicState> =
            vec![vk::DynamicState::VIEWPORT, vk::DynamicState::SCISSOR];
        let dynamic_info = vk::PipelineDynamicStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_DYNAMIC_STATE_CREATE_INFO,
            p_next: ptr::null(),
            flags: vk::PipelineDynamicStateCreateFlags::empty(),
            dynamic_state_count: dynamic_states.len() as u32,
            p_dynamic_states: dynamic_states.as_ptr(),
            _marker: Default::default(),
        };
        let entry = "main\0".as_ptr() as *const c_char;
        let shader_stages = self
            .shaders
            .iter()
            .map(|(stage, shader)| vk::PipelineShaderStageCreateInfo {
                s_type: vk::StructureType::PIPELINE_SHADER_STAGE_CREATE_INFO,
                p_next: ptr::null(),
                flags: vk::PipelineShaderStageCreateFlags::empty(),
                stage: *stage,
                module: shader.handle(),
                p_name: entry,
                p_specialization_info: ptr::null(),
                _marker: Default::default(),
            })
            .collect::<Vec<vk::PipelineShaderStageCreateInfo>>();
        self.render_info.p_color_attachment_formats = &self.color_attachment_format;

        let pipeline_info = vk::GraphicsPipelineCreateInfo {
            s_type: vk::StructureType::GRAPHICS_PIPELINE_CREATE_INFO,
            p_next: &self.render_info as *const _ as *const c_void,
            flags: vk::PipelineCreateFlags::empty(),
            stage_count: self.shaders.len() as u32,
            p_stages: shader_stages.as_ptr(),
            p_vertex_input_state: &vertex_input_info,
            p_input_assembly_state: &self.input_assembly,
            p_tessellation_state: ptr::null(),
            p_viewport_state: &viewport_state,
            p_rasterization_state: &self.rasterizer,
            p_multisample_state: &self.multisampling,
            p_depth_stencil_state: &self.depth_stencil,
            p_color_blend_state: &color_blending,
            p_dynamic_state: &dynamic_info,
            layout: self.layout.unwrap(),
            render_pass: vk::RenderPass::null(),
            subpass: 0,
            base_pipeline_handle: vk::Pipeline::null(),
            base_pipeline_index: 0,
            _marker: Default::default(),
        };

        let handle = unsafe {
            device
                .get_handle()
                .create_graphics_pipelines(vk::PipelineCache::null(), &[pipeline_info], None)
                .unwrap()
        }
        .pop()
        .unwrap();
        // Clean up shaders
        for shader in self.shaders.into_values() {
            drop(shader)
        }

        Ok(Self::BuildTo { handle, device })
    }
}

impl GraphicsPipelineBuilder<'_> {
    /// Clears all currently held layouts + shaders but does not delete them.
    pub fn clear(self) -> Self {
        Self::default()
    }

    pub fn set_input_topology(mut self, topology: vk::PrimitiveTopology) -> Self {
        self.input_assembly.topology = topology;
        self.input_assembly.primitive_restart_enable = vk::FALSE;
        self
    }

    pub fn set_polygon_mode(mut self, poly_mode: vk::PolygonMode) -> Self {
        self.rasterizer.polygon_mode = poly_mode;
        self.rasterizer.line_width = 1.0f32;
        self
    }

    pub fn set_cull_mode(
        mut self,
        cull_mode: vk::CullModeFlags,
        front_face: vk::FrontFace,
    ) -> Self {
        self.rasterizer.cull_mode = cull_mode;
        self.rasterizer.front_face = front_face;
        self
    }

    pub fn set_multisampling_none(mut self) -> Self {
        self.multisampling.sample_shading_enable = vk::FALSE;
        self.multisampling.rasterization_samples = vk::SampleCountFlags::TYPE_1;
        self.multisampling.min_sample_shading = 1.0f32;
        self.multisampling.p_sample_mask = ptr::null();
        self.multisampling.alpha_to_coverage_enable = vk::FALSE;
        self.multisampling.alpha_to_one_enable = vk::FALSE;
        self
    }

    pub fn disable_blending(mut self) -> Self {
        self.color_blend_attachment.color_write_mask = vk::ColorComponentFlags::RGBA;
        self.color_blend_attachment.blend_enable = vk::FALSE;
        self
    }

    pub fn set_color_attachment(mut self, format: vk::Format) -> Self {
        self.color_attachment_format = format;
        self.render_info.color_attachment_count = 1;
        self
    }

    pub fn set_depth_format(mut self, format: vk::Format) -> Self {
        self.render_info.depth_attachment_format = format;
        self
    }

    pub fn disable_depth_test(mut self) -> Self {
        self.depth_stencil.depth_test_enable = vk::FALSE;
        self.depth_stencil.depth_write_enable = vk::FALSE;
        self.depth_stencil.depth_compare_op = vk::CompareOp::NEVER;
        self.depth_stencil.stencil_test_enable = vk::FALSE;
        self.depth_stencil.front = Default::default();
        self.depth_stencil.back = Default::default();
        self.depth_stencil.min_depth_bounds = 0.0f32;
        self.depth_stencil.max_depth_bounds = 1.0f32;
        self
    }

    pub fn enable_depth_test(
        mut self,
        depth_write_enable: vk::Bool32,
        compare_op: vk::CompareOp,
    ) -> Self {
        self.depth_stencil.depth_test_enable = vk::TRUE;
        self.depth_stencil.depth_write_enable = depth_write_enable;
        self.depth_stencil.depth_compare_op = compare_op;
        self.depth_stencil.depth_bounds_test_enable = vk::FALSE;
        self.depth_stencil.stencil_test_enable = vk::FALSE;
        self.depth_stencil.front = Default::default();
        self.depth_stencil.back = Default::default();
        self.depth_stencil.min_depth_bounds = 0.0f32;
        self.depth_stencil.max_depth_bounds = 1.0f32;
        self
    }

    pub fn color_blending(mut self, blending: vk::PipelineColorBlendAttachmentState) -> Self {
        self.color_blend_attachment = blending;
        self
    }

    pub fn enable_blending_additive(mut self) -> Self {
        self.color_blend_attachment = vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::RGBA,
        };
        self
    }

    pub fn enable_blending_alpha_blend(mut self) -> Self {
        self.color_blend_attachment = vk::PipelineColorBlendAttachmentState {
            blend_enable: vk::TRUE,
            src_color_blend_factor: vk::BlendFactor::SRC_ALPHA,
            dst_color_blend_factor: vk::BlendFactor::ONE_MINUS_SRC_ALPHA,
            color_blend_op: vk::BlendOp::ADD,
            src_alpha_blend_factor: vk::BlendFactor::ONE,
            dst_alpha_blend_factor: vk::BlendFactor::ZERO,
            alpha_blend_op: vk::BlendOp::ADD,
            color_write_mask: vk::ColorComponentFlags::RGBA,
        };
        self
    }
}

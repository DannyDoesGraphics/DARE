use std::ops::Deref;
use std::ptr;
use ash::vk;
use crate::command::command_buffer::CmdBuffer;

/// Contains the dynamic render context which contains references to the original command buffer
#[derive(Debug)]
pub struct DynamicRenderContext<'a> {
    handle: &'a crate::command::CommandBufferRecording,
    color_attachments: Vec<vk::RenderingAttachmentInfo<'a>>,
    depth_attachment: Option<vk::RenderingAttachmentInfo<'a>>,
}

impl<'a> DynamicRenderContext<'a> {
    /// Create a new vk object from a VkObjects. This is internal use only.
    pub(crate) fn from_vk(handle: &'a crate::command::CommandBufferRecording) -> Self {
        Self {
            handle,
            color_attachments: Vec::new(),
            depth_attachment: None,
        }
    }

    /// Pushes an image into the dynamic render as a color attachment
    pub fn push_image_as_color_attachment(mut self, image_layout: vk::ImageLayout, image_view: &crate::resource::ImageView, clear_value: Option<vk::ClearValue>) -> Self {
        self.color_attachments.push(vk::RenderingAttachmentInfo {
            s_type: vk::StructureType::RENDERING_ATTACHMENT_INFO,
            p_next: ptr::null(),
            image_view: image_view.handle(),
            image_layout,
            load_op: match clear_value {
                None => vk::AttachmentLoadOp::LOAD,
                Some(_) => vk::AttachmentLoadOp::CLEAR
            },
            store_op: vk::AttachmentStoreOp::STORE,
            clear_value: clear_value.unwrap_or_default(),
            ..Default::default()
        });
        self
    }

    /// Begins rendering
    pub fn begin_rendering(self, extent: vk::Extent2D) -> Self {
        let render_info = vk::RenderingInfo {
            s_type: vk::StructureType::RENDERING_INFO,
            p_next: ptr::null(),
            flags: vk::RenderingFlags::empty(),
            render_area: vk::Rect2D {
                offset: vk::Offset2D {
                    x: 0,
                    y: 0,
                },
                extent,
            },
            layer_count: 1,
            view_mask: 0,
            color_attachment_count: self.color_attachments.len() as u32,
            p_color_attachments: self.color_attachments.as_ptr(),
            p_depth_attachment: match self.depth_attachment.as_ref() {
                None => ptr::null(),
                Some(attachment) => attachment
            },
            p_stencil_attachment: ptr::null(),
            _marker: Default::default(),
        };
        unsafe {
            self.handle.get_device().get_handle().cmd_begin_rendering(self.handle.handle(), &render_info);
        }
        self
    }

    /// Ends rendering
    pub fn end_rendering(mut self) {
        unsafe {
            self.handle.get_device().get_handle().cmd_end_rendering(self.handle.handle());
        }
    }
}

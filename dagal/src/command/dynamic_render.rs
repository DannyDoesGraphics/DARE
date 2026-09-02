use ash::vk;

use crate::command::command_buffer::CmdBuffer;
use crate::traits::AsRaw;

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
    pub fn push_image_as_color_attachment(
        mut self,
        image_layout: vk::ImageLayout,
        image_view: &crate::resource::ImageView,
        clear_value: Option<vk::ClearValue>,
    ) -> Self {
        self.color_attachments.push(
            vk::RenderingAttachmentInfo::default()
                .image_view(unsafe { *image_view.as_raw() })
                .image_layout(image_layout)
                .load_op(match clear_value {
                    None => vk::AttachmentLoadOp::LOAD,
                    Some(_) => vk::AttachmentLoadOp::CLEAR,
                })
                .store_op(vk::AttachmentStoreOp::STORE)
                .clear_value(clear_value.unwrap_or_default()),
        );
        self
    }

    /// Begins rendering
    pub fn begin_rendering(self, extent: vk::Extent2D) -> Self {
        let render_info = vk::RenderingInfo::default()
            .render_area(vk::Rect2D {
                offset: vk::Offset2D { x: 0, y: 0 },
                extent,
            })
            .layer_count(1)
            .color_attachments(&self.color_attachments);
        let render_info = match self.depth_attachment.as_ref() {
            None => render_info,
            Some(attachment) => render_info.depth_attachment(attachment),
        };
        unsafe {
            self.handle
                .get_device()
                .get_handle()
                .cmd_begin_rendering(*self.handle.as_raw(), &render_info);
        }
        self
    }

    pub fn depth_attachment_info(
        mut self,
        image_view: vk::ImageView,
        image_layout: vk::ImageLayout,
    ) -> Self {
        let depth_attachment = vk::RenderingAttachmentInfo::default()
            .image_view(image_view)
            .image_layout(image_layout)
            .load_op(vk::AttachmentLoadOp::CLEAR)
            .store_op(vk::AttachmentStoreOp::STORE)
            .clear_value(vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue {
                    depth: 0.0,
                    stencil: 0,
                },
            });
        self.depth_attachment = Some(depth_attachment);
        self
    }

    /// Ends rendering
    pub fn end_rendering(self) {
        unsafe {
            self.handle
                .get_device()
                .get_handle()
                .cmd_end_rendering(*self.handle.as_raw());
        }
    }
}

use ash::vk;
use std::ptr;

pub fn rendering_info<'a>(
    render_extent: vk::Extent2D,
    color_attachment: &'a [vk::RenderingAttachmentInfo],
    depth_attachment: Option<&'a vk::RenderingAttachmentInfo>,
) -> vk::RenderingInfo<'a> {
    vk::RenderingInfo {
        s_type: vk::StructureType::RENDERING_INFO,
        p_next: ptr::null(),
        flags: vk::RenderingFlags::empty(),
        render_area: vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: render_extent,
        },
        layer_count: 1,
        view_mask: 0,
        color_attachment_count: 1,
        p_color_attachments: color_attachment.as_ptr(),
        p_depth_attachment: match depth_attachment {
            None => ptr::null(),
            Some(depth_attachment) => depth_attachment,
        },
        p_stencil_attachment: ptr::null(),
        _marker: Default::default(),
    }
}

use ash::vk;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceAccess {
    pub stage: vk::PipelineStageFlags2,
    pub access: vk::AccessFlags2,
    pub layout: vk::ImageLayout,
}

impl ResourceAccess {
    pub const fn new(
        stage: vk::PipelineStageFlags2,
        access: vk::AccessFlags2,
        layout: vk::ImageLayout,
    ) -> Self {
        Self {
            stage,
            access,
            layout,
        }
    }

    /// Create an access to a buffer
    pub const fn buffer(stage: vk::PipelineStageFlags2, access: vk::AccessFlags2) -> Self {
        Self::new(stage, access, vk::ImageLayout::UNDEFINED)
    }

    /// Generic read over any resource
    pub const fn general() -> Self {
        Self::new(
            vk::PipelineStageFlags2::ALL_COMMANDS,
            vk::AccessFlags2::from_raw(
                vk::AccessFlags2::MEMORY_READ.as_raw() | vk::AccessFlags2::MEMORY_WRITE.as_raw(),
            ),
            vk::ImageLayout::GENERAL,
        )
    }

    pub const fn color_write() -> Self {
        Self::new(
            vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT,
            vk::AccessFlags2::COLOR_ATTACHMENT_WRITE,
            vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
        )
    }

    pub const fn depth_stencil_write() -> Self {
        Self::new(
            vk::PipelineStageFlags2::from_raw(
                vk::PipelineStageFlags2::EARLY_FRAGMENT_TESTS.as_raw()
                    | vk::PipelineStageFlags2::LATE_FRAGMENT_TESTS.as_raw(),
            ),
            vk::AccessFlags2::DEPTH_STENCIL_ATTACHMENT_WRITE,
            vk::ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
        )
    }

    pub const fn sampled(stage: vk::PipelineStageFlags2) -> Self {
        Self::new(
            stage,
            vk::AccessFlags2::SHADER_SAMPLED_READ,
            vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        )
    }

    pub const fn sampled_fragment() -> Self {
        Self::sampled(vk::PipelineStageFlags2::FRAGMENT_SHADER)
    }

    pub const fn storage(stage: vk::PipelineStageFlags2) -> Self {
        Self::new(
            stage,
            vk::AccessFlags2::from_raw(
                vk::AccessFlags2::SHADER_STORAGE_READ.as_raw()
                    | vk::AccessFlags2::SHADER_STORAGE_WRITE.as_raw(),
            ),
            vk::ImageLayout::GENERAL,
        )
    }

    pub const fn transfer_read() -> Self {
        Self::new(
            vk::PipelineStageFlags2::ALL_TRANSFER,
            vk::AccessFlags2::TRANSFER_READ,
            vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
        )
    }

    pub const fn transfer_write() -> Self {
        Self::new(
            vk::PipelineStageFlags2::ALL_TRANSFER,
            vk::AccessFlags2::TRANSFER_WRITE,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        )
    }

    pub fn union(self, other: Self) -> Self {
        let layout = if self.layout == other.layout {
            self.layout
        } else {
            vk::ImageLayout::GENERAL
        };
        Self {
            stage: self.stage | other.stage,
            access: self.access | other.access,
            layout,
        }
    }
}

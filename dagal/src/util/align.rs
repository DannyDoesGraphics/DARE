use ash::vk;

/// Deals with aligning data

pub fn align(current: vk::DeviceSize, alignment: vk::DeviceSize) -> vk::DeviceSize {
    if alignment == 0 || current % alignment == 0 {
        current
    } else {
        let remainder = current % alignment;
        current + alignment - remainder
    }
}
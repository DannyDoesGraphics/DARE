/// Responsible for loading gltf assets

pub struct GltfLoader {
    immediate: dagal::util::ImmediateSubmit,
}

impl GltfLoader {
    pub fn new(immediate: dagal::util::ImmediateSubmit) -> Self {
        Self { immediate }
    }
}

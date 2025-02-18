use crate::prelude as dare;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Sampler {
    /// Wrapping mode (s,t)
    pub wrapping_mode: (
        dare::render::util::WrappingMode,
        dare::render::util::WrappingMode,
    ),
    pub min_filter: dare::render::util::ImageFilter,
    pub mag_filter: dare::render::util::ImageFilter,
}

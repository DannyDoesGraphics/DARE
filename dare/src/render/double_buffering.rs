/// The idea is to allow reads to occur without interrupting the rendering tasks at hand by having
/// 2 buffers, A and B
///
/// Let A be the buffer that is being read for rendering
/// Let B be the buffer that is being 
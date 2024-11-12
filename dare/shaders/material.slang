struct Material {
    uint32_t bit_flag;
    float4 color_factor;
    uint32_t albedo_texture_id;
    uint32_t albedo_sampler_id;
    uint32_t normal_texture_id;
    uint32_t normal_sampler_id;
}
enum MaterialFlags : uint32_t {
    NONE = 0 << 0,
    ALBEDO = 1 << 0,
    NORMAL = 1 << 2,
}
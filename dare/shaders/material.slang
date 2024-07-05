struct Material {
    uint bit_flag;
    float4 color_factor;
    uint albedo_texture_id;
    uint albedo_sampler_id;
    uint normal_texture_id;
    uint normal_sampler_id;
}
enum MaterialFlags : uint {
    NONE = 0 << 0,
    ALBEDO = 1 << 0,
    NORMAL = 1 << 2,
}
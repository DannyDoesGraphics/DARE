
struct Material {
    uint32_t bit_flag;
    uint32_t _padding;
    float4 color_factor;
    uint32_t albedo_texture_id;
    uint32_t albedo_sampler_id;
    uint32_t normal_texture_id;
    uint32_t normal_sampler_id;
};
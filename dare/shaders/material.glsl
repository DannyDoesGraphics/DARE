#include <dagal/ext.glsl>
/// Represents a [`CMaterial`] struct

layout(buffer_reference, scalar) readonly buffer Material {
    uint bit_flag;
    vec4 color_factor;
    uint albedo_texture_id;
    uint albedo_sampler_id;
    uint normal_texture_id;
    uint normal_sampler_id;
};
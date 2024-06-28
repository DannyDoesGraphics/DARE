#version 430
#include "bindless.glsl"
#include "raster/push_constants.glsl"
#include "surface_bit_flags.glsl"

layout (location = 0) in vec2 in_uv;

layout (location = 0) out vec4 out_color;

void main() {
    vec4 tex_color = vec4(1.0);
    if (is_flag_set(pc.surface.bit_flag, SURFACE_UV_BIT) && is_flag_set(pc.surface.material.bit_flag, MATERIAL_ALBEDO_BIT)) {
        tex_color = vec4(texture(
        sampler2D(sampled_images[pc.surface.material.albedo_texture_id], samplers[pc.surface.material.albedo_sampler_id]),
        in_uv
        ).rgba);
    }
    out_color = pc.surface.material.color_factor * tex_color * vec4(1.0);
}
#version 430
#include "bindless.glsl"
#include "raster/push_constants.glsl"
#include "surface_bit_flags.glsl"

layout (location = 0) out vec2 out_uv;

void main() {
    vec4 position = vec4(pc.surface.positions.vectors[gl_VertexIndex], 1.0);
    gl_Position = pc.scene_data.view_proj * pc.model_transform * position;
    if (is_flag_set(pc.surface.bit_flag, SURFACE_UV_BIT)) {
        out_uv = pc.surface.uv.vectors[gl_VertexIndex];
    }
}
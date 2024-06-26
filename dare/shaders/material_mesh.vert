#version 430
#include "bindless.glsl"
#include "raster/push_constants.glsl"

layout (location = 0) out vec3 out_normal;
layout (location = 1) out vec4 vertex_color_factor;
layout (location = 2) out vec2 out_uv;

void main() {
    vec4 position = vec4(pc.surface.positions.vectors[gl_VertexIndex], 1.0);
    gl_Position = pc.scene_data.view_proj * pc.model_transform * position;

    vertex_color_factor = vec4(1.0);
}
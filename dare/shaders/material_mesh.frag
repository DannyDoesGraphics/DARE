#version 430
#include "bindless.glsl"
#include "raster/push_constants.glsl"

layout (location = 0) in vec3 out_normal;
layout (location = 1) in vec4 vertex_color_factor;
layout (location = 2) in vec2 out_uv;

layout (location = 0) out vec4 out_color;

void main() {
    out_color = vertex_color_factor * vec4(1.0);
}
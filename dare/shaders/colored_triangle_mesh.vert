#version 450
#include <dagal/ext.glsl>
#include <dagal/dagal.glsl>

layout(set = 0, binding = 0) uniform sampler samplers;
layout(set = 0, binding = 1) uniform texture2D sampled_images[];
layout(set = 0, binding = 2, r32f) uniform image2D storage_images[];
layout(set = 0, binding = 3) readonly buffer BDA {
    u64 buffer_addresses[];
};

layout (location = 0) out vec3 outColor;
layout (location = 1) out vec2 outUV;

struct Vertex {
    vec3 position;
    f32 uv_x;
    vec3 normal;
    f32 uv_y;
    vec4 color;
};

layout(buffer_reference, std430) readonly buffer VertexBuffer{
    Vertex vertices[];
};

//push constants block
layout( push_constant ) uniform constants
{
    mat4 render_matrix;
    u32 vertex_buffer_id;
} pc;

void main()
{
    //load vertex data from device adress
    VertexBuffer vertex_buffer = VertexBuffer(buffer_addresses[pc.vertex_buffer_id]);
    Vertex v = vertex_buffer.vertices[gl_VertexIndex];

    //output data
    gl_Position = pc.render_matrix * vec4(v.position, 1.0f);
    outColor = v.color.xyz;
    outUV.x = v.uv_x;
    outUV.y = v.uv_y;
}
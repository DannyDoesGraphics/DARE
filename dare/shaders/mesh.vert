#include <dagal/ext.glsl>
#include <dagal/dagal.glsl>
#include "constants.glsl"

layout(set = 0, binding = 0) uniform sampler samplers[];
layout(set = 0, binding = 1) uniform texture2D sampled_images[];
layout(set = 0, binding = 2, r32f) uniform image2D storage_images[];
layout(set = 0, binding = 3) readonly buffer BDA {
    uint64_t buffer_addresses[];
};

layout (location = 0) out vec3 outNormal;
layout (location = 1) out vec3 outColor;
layout (location = 2) out vec2 outUV;

struct Vertex {
    vec3 position;
    float uv_x;
    vec3 normal;
    float uv_y;
    vec4 color;
};

layout(buffer_reference, std430) readonly buffer VertexBuffer {
    Vertex vertices[];
};

void main()
{
    Vertex v = VertexBuffer(buffer_addresses[pc.vertex_buffer_index])[gl_VertexIndex];
    SceneData scene_data = SceneData(buffer_addresses[pc.scene_data_index]);
    GLTFMaterialData material_data = GLTFMaterialData(buffer_addresses[pc.gltf_material_data_index]);

    vec4 position = vec4(v.position, 1.0f);

    gl_Position =  scene_data.view_proj * pc.render_matrix * position;

    outNormal = (pc.render_matrix * vec4(v.normal, 0.f)).xyz;
    outColor = v.color.xyz * material_data.color_factors.xyz;
    outUV.x = v.uv_x;
    outUV.y = v.uv_y;
}
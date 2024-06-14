#version 450
#include <dagal/ext.glsl>
#include <dagal/dagal.glsl>
#include "constants.glsl"

layout(set = 0, binding = 0) uniform sampler samplers[];
layout(set = 0, binding = 1) uniform texture2D sampled_images[];
layout(set = 0, binding = 2, r32f) uniform image2D storage_images[];
layout(set = 0, binding = 3) readonly buffer BDA {
    uint64_t buffer_addresses[];
};

layout (location = 0) in vec3 inNormal;
layout (location = 1) in vec3 inColor;
layout (location = 2) in vec2 inUV;

layout (location = 0) out vec4 outFragColor;

void main()
{
    GLTFMaterialData material_data = GLTFMaterialData(buffer_addresses[pc.material_buffer_index]);


    vec3 color = inColor * texture(sampler2D(sampled_images[material_data.color_image], samplers[material_data.color_image_sampler]), inUV).xyz;

    outFragColor = vec4(color, 1.0f);
}
#version 450
#include <dagal/ext.glsl>

//shader input
layout (location = 0) in vec3 inColor;
layout (location = 1) in vec2 inUV;

//output write
layout (location = 0) out vec4 outFragColor;

// bindless
layout(set = 0, binding = 0) uniform sampler samplers[];
layout(set = 0, binding = 1) uniform texture2D sampled_images[];
layout(set = 0, binding = 2, r32f) uniform image2D storage_images[];
layout(set = 0, binding = 3) readonly buffer BDA {
    uint64_t buffer_addresses[];
};

void main()
{
    vec4 col = texture(sampler2D(sampled_images[7], samplers[0]), inUV);
    outFragColor = vec4(col);
    //outFragColor = vec4(1.0);
}
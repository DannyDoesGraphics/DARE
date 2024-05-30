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
    SceneData scene_data = SceneData(buffer_addresses[constants.scene_data_index]);
    float lightValue = max(dot(inNormal, scene_data.sunlight_direction.xyz), 0.1f);


    vec3 color = inColor * texture(sampler2D(sampled_images[pc.sampled_image_index], samplers[pc.sampler_index]), inUV).xyz;
    vec3 ambient = color *  scene_data.ambient_color.xyz;

    outFragColor = vec4(color * lightValue *  scene_data.sun_light_color.w + ambient, 1.0f);
}
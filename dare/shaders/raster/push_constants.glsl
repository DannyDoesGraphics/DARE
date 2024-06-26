#include "../scene_data.glsl"
#include "../surface.glsl"

layout(push_constant) uniform PushConstantRaster {
    SceneData scene_data;
    Surface surface;
    mat4 model_transform;
} pc;
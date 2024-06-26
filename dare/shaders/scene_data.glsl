#include <dagal/ext.glsl>

layout (buffer_reference, scalar) readonly buffer SceneData {
    mat4 view;
    mat4 proj;
    mat4 view_proj;
};
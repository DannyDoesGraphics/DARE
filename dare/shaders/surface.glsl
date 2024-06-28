#include <dagal/ext.glsl>
#include "material.glsl"

layout (buffer_reference, scalar) readonly buffer Vec3Array {
    vec3 vectors[];
};
layout (buffer_reference, scalar) readonly buffer Vec2Array {
    vec2 vectors[];
};

layout (buffer_reference, scalar) readonly buffer Surface {
    Material material;
    mat4 transform;
    uint bit_flag;
    Vec3Array positions;
    Vec3Array indices;
    Vec3Array normals;
    Vec3Array tangents;
    Vec2Array uv;
};
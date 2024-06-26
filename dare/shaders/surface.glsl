#include <dagal/ext.glsl>
#include "material.glsl"

layout (buffer_reference, scalar) readonly buffer Vec3Array {
    vec3 vectors[];
};

layout (buffer_reference, scalar) readonly buffer Surface {
    Material material;
    mat4 transform;
    uint bit_flags;
    uint _padding;
    Vec3Array positions;
    Vec3Array indices;
    Vec3Array normals;
    Vec3Array tangents;
    Vec3Array uv;
};
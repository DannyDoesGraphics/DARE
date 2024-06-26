#include <dagal/ext.glsl>
/// Represents a [`CMaterial`] struct

layout(buffer_reference, scalar) readonly buffer Material {
    uint texture_flags;
    vec4 color_factor;
    uint albedo_texture_id;
    uint normal_texture_id;
};
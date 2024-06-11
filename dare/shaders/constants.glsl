layout(buffer_reference, std430) readonly buffer SceneData {
    mat4 view;
    mat4 proj;
    mat4 view_proj;
    vec4 ambient_color;
    vec4 sunlight_direction;
    vec4 sunlight_color;
};

layout(buffer_reference, std430) readonly buffer GLTFMaterialData {
    vec4 color_factors;
    vec4 metal_rough_factors;

    uint color_image;
    uint color_image_sampler;

    uint metal_rough_image;
    uint metal_rough_image_sampler;
};

layout(push_constant) uniform PushConstant {
    uint material_buffer_index;
    uint scene_data_index;
    uint vertex_buffer_index;
    mat4 render_matrix;
} pc;
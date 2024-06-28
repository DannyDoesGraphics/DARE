const uint SURFACE_NORMAL_BIT = 1 << 0;
const uint SURFACE_TANGENT_BIT = 1 << 1;
const uint SURFACE_UV_BIT = 1 << 2;

const uint MATERIAL_ALBEDO_BIT = 1 << 0;
const uint MATERIAL_NORMAL_BIT = 1 << 1;

/// Checks if a given bit flag is set
bool is_flag_set(in uint bit, in uint flag) {
    return (bit & flag) == flag;
}
#include "material.slang"

struct Surface {
    // information used for aabb plane test
    const float4x4 transform;
    const float3 min;
    const float3 max;
    // information used for rendering
    const uint64_t material;
    const uint32_t bit_flag;
    const uint32_t _padding;
    const float3* positions;
    const float3* indices;
    const float3* normals;
    const float3* tangents;
    const float2* uv;
}
enum SurfaceFlags : uint {
    NONE = 0x0,
    NORMAL = 1 << 0,
    TANGENT = 1 << 1,
    UV = 1 << 2,
}
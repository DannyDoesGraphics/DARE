#include "material.slang"

struct Mesh {
    const Material* material;
    const uint bit_flag;
    const float3* positions;
    const float3* indices;
    const float3* normals;
    const float3* tangents;
    const float2* uv;
    const float4x4 transform;
}

struct Surface {
    const Material* material;
    const uint bit_flag;
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
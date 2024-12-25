#include "material.slang"

struct Surface {
    const uint64_t material;
    const uint bit_flag;
    const uint _padding;
    const Ptr<float3> positions;
    const Ptr<float3> indices;
    const Ptr<float3> normals;
    const Ptr<float3> tangents;
    const Ptr<float2> uv;
}
enum SurfaceFlags : uint {
    NONE = 0x0,
    NORMAL = 1 << 0,
    TANGENT = 1 << 1,
    UV = 1 << 2,
}
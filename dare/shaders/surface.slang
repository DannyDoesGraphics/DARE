#include "material.slang"

/**
* Struct of arrays representation of surfaces
*/
struct SurfaceArray {
    uint64_t length;
    Ptr<float4x4[], Access.Read> transforms;
    Ptr<float3[], Access.Read> minimums;
    Ptr<float3[], Access.Read> maximums;
    Ptr<uint64_t[], Access.Read> materials;
    Ptr<SurfaceFlags[], Access.Read> bit_flags;
    Ptr<uint32_t[], Access.Read> index_counts;
    Ptr<Ptr<float3[], Access.Read>[]> positions;
    Ptr<Ptr<float3[], Access.Read>[]> indices;
    Ptr<Ptr<float3[], Access.Read>[]> normals;
    Ptr<Ptr<float3[], Access.Read>[]> tangents;
    Ptr<Ptr<float2[], Access.Read>[]> uv;
};

struct Surface {
    // information used for aabb plane test
    const float4x4 transform;
    const float3 min;
    const float3 max;
    // information used for rendering
    const uint64_t material;
    const uint32_t bit_flag;
    const uint32_t index_count;
    Ptr<float3, Access.Read> positions;
    Ptr<float3, Access.Read> indices;
    Ptr<float3, Access.Read> normals;
    Ptr<float3, Access.Read> tangents;
    Ptr<float2, Access.Read> uv;
}
enum SurfaceFlags : uint {
    NONE = 0x0,
    NORMAL = 1 << 0,
    TANGENT = 1 << 1,
    UV = 1 << 2,
}
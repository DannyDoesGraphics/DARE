//! Handles culling of surfaces

#include "surface.slang"
#include "gpu_rendering.slang"

struct PushConstant {
    const float4x4 view_proj;
    const uint64_t surface_count;
   Ptr<InstancedSurfacesInfo, Access.Read> instanced_surface_info;
   Ptr<Surface, Access.Read> surface_infos;
};

struct Plane {
  float3 normal;
  float d;
};

void extract_frustum_plane(float4x4 view_proj, out Plane planes[6]) {
    float4 r0 = view_proj[0];
    float4 r1 = view_proj[1];
    float4 r2 = view_proj[2];
    float4 r3 = view_proj[3];

    // left, right, bottom, top, near, far
    float4 raw[6] = {
        r3 + r0,
        r3 - r0,
        r3 + r1,
        r3 - r1,
        r3 + r2,
        r3 - r2
    };

    // normalize planes
    for (int i = 0; i < 6; i++) {
        float len = length(raw[i].xyz);
        planes[i].normal = raw[i].xyz / len;
        planes[i].d = raw[i].w / len;
    }
}

bool aabb_in_frustum(
    Plane plane[6],
    float3 min,
    float3 max
) {
    for (int i = 0; i < 6; i++) {
        Plane p = plane[i];
        float3 positive = float3(
            p.normal.x > 0 ? max.x : min.x,
            p.normal.y > 0 ? max.y : min.y,
            p.normal.z > 0 ? max.z : min.z
        );

        if (dot(p.normal, positive) + p.d < 0) {
            return false;
        }
    }
    return true;
}

[vk::binding(0, 0)] RWStructuredBuffer<uint> cull_mask;
[vk::push_constant] PushConstant pc;
[shader("compute")]
[numthreads(32,1,1)]
void main(uint3 dispatch_id: SV_DispatchThreadID, uint3 group_id: SV_GroupID, uint3 group_tid: SV_GroupThreadID) {
    uint32_t idx = dispatch_id.x;
    if (idx >= pc.surface_count) {
        return;
    }

    const Surface surface = pc.surface_infos[idx];
    // aa-bb plane test
    Plane frustum[6];
    extract_frustum_plane(pc.view_proj, frustum);
    bool visible = aabb_in_frustum(frustum, surface.min, surface.max);

    // ballot mask
    uint4 ballot_mask = WaveActiveBallot(visible);
    // gather visibility across waves
    uint4 wave_mask_4 = WaveActiveBallot(visible);
    uint wave_mask = wave_mask_4.x;

    if (WaveGetLaneIndex() == 0) {
        // compute byte offset: one 32-bit word per lane
        uint32_t wave_size = WaveGetLaneCount();
        uint32_t wave_number = dispatch_id.x / wave_size;
        // cover full subgroup width
        cull_mask[wave_number * 4 + 0] = ballot_mask.x;
        cull_mask[wave_number * 4 + 1] = ballot_mask.y;
        cull_mask[wave_number * 4 + 2] = ballot_mask.z;
        cull_mask[wave_number * 4 + 3] = ballot_mask.w;
    }
}
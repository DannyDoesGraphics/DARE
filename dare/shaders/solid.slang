#include "random.slang"
#include "gpu_rendering.slang"
#extension VK_EXT_debug_printf : enable

struct FSin {
    uint32_t rand;
};
struct VSout {
    FSin fragment_in;
    float4 sv_position: SV_Position;
};
struct FSout {
    float4 color: SV_Target;
};
struct PushConstant {
    const float4x4 view_proj;
    const InstancedSurfacesInfo *instanced_surface_info;
    const Surface *surface_infos;
    const float4x4 *transforms;
    const uint64_t draw_id;
};

float convertUintToFloat(uint value)
{
    // Define the maximum representable float value

    // Convert uint to float
    float result = float(value);

    // Clamp the result to the maximum float value
    return value;
};

[[vk::push_constant]] PushConstant pc;
/// Renders each mesh out as a singular solid
[shader("vertex")]
VSout vertex_main(
    uint vertex_index: SV_VertexID,  // index buffer
    uint instance_id: SV_InstanceID, // current draw instance id
) {
    //printf("ids: %u - %u\n", pc.instanced_surface_info.surface.bit_flag, pc.instanced_surface_info.instances);

    // Print the high and low parts as hexadecimal
    const InstancedSurfacesInfo instanced_info = pc.instanced_surface_info[0];
    const Surface surface_info = pc.surface_infos[instanced_info.surface];
    VSout out;
    float3 vertex = float3(surface_info.positions[vertex_index]);
    float4 local_position = float4(vertex, 1.0);
    float4x4 instance_transform = pc.transforms[instanced_info.instances_offset + instance_id];
    float4 world_position = mul(local_position, instance_transform);

    float4 clip_space = mul(pc.view_proj, world_position);
    out.sv_position = clip_space;

    FSin f_in;
    f_in.rand = uint(pc.draw_id);

    out.fragment_in = f_in;
    return out;
}

[shader("fragment")]
FSout fragment_main(FSin stage) {
    FSout out;
    out.color = float4(rnd(stage.rand), rnd(stage.rand), rnd(stage.rand), 1.0);
    return out;
}
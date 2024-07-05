#include "bindless.slang"
#include "surface.slang"
#include "scene_data.slang"

struct PushConstant {
    const SceneData* scene_data;
    const Surface* surface;
    float4x4 model_transform;
}

struct FSin {
    float2 uv;
}
struct VSout {
    FSin stage;
    float4 sv_position: SV_Position;
}
struct FSout {
    float4 color: SV_Target;
}

[[vk::push_constant]] PushConstant pc;

[shader("vertex")]
VSout vertex_main(uint vertex_index: SV_VertexID) {
    float4 position = float4(pc.surface.positions[vertex_index], 1.0);
    VSout output;
    output.sv_position = mul(mul(position, pc.model_transform), pc.scene_data.view_proj);
    if ((pc.surface.bit_flag & SurfaceFlags.UV) == SurfaceFlags.UV) {
        output.stage.uv = pc.surface.uv[vertex_index];
    }
    
    return output;
}

[shader("fragment")]
FSout fragment_main(FSin stage) {
    FSout output;
    output.color = float4(1.0);
    if ((pc.surface.bit_flag & SurfaceFlags.UV) == SurfaceFlags.UV) {
        if ((pc.surface.material.bit_flag & MaterialFlags.ALBEDO) == MaterialFlags.ALBEDO) {
            output.color = sampled_images[pc.surface.material.albedo_texture_id].Sample(
                samplers[pc.surface.material.albedo_sampler_id],
                stage.uv
            );
        }
    }
    output.color *= pc.surface.material.color_factor;
    return output;
}
#include "random.slang"

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
    const float3* vertices;
};

[[vk::push_constant]] PushConstant pc;
/// Renders each mesh out as a singular solid
[shader("vertex")]
VSout vertex_main(uint vertex_index: SV_VertexID) {
    VSout out;
    float3 positions[3] = {
        float3(-0.5f, -0.5f, 0.0f),  // Vertex 0
        float3(0.0f,  0.5f, 0.0f),   // Vertex 1
        float3(0.5f, -0.5f, 0.0f)    // Vertex 2
    };
    out.sv_position = mul(float4(pc.vertices[vertex_index], 1.0), pc.view_proj);
    out.sv_position = mul(pc.view_proj, float4(pc.vertices[vertex_index], 1.0));
    //out.sv_position = float4(pc.vertices[vertex_index], 1.0);
    //out.sv_position = float4(positions[vertex_index % 3], 1.0);
    //out.sv_position = float4(pc.vertices[vertex_index], 1.0);
    FSin f_in;
    //f_in.rand = (uint32_t)determinant(pc.view_proj);
    f_in.rand = vertex_index;
    out.fragment_in = f_in;
    return out;
}

[shader("fragment")]
FSout fragment_main(FSin stage) {
    FSout out;
    out.color = float4(rnd(stage.rand), rnd(stage.rand), rnd(stage.rand), 1.0);
    return out;
}
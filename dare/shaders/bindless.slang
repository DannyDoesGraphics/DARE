[[vk::binding(0,0)]]
SamplerState samplers[];

[[vk::binding(1,0)]]
Texture2D<float4> sampled_images[];

[[vk::binding(2,0)]]
RWTexture2D<float4> storage_images[];

[[vk::binding(3,0)]]
StructuredBuffer<uint64_t, ScalarDataLayout> buffer_addresses;


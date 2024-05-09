# DARE
**D**anny's **A**wesome's **R**endering **E**ngine (DARE) is a simple path tracer that utilizes Vulkan underneath the 
the hood to handle graphics. You'll notice it's a very thin abstraction layer and that is by design (KISS). Where 
possible, we prefer to use Vulkan objects over our own primitives but due to DX, we may opt to use
our own objects such as storing the `VkDevice`.


## Requirements
We expect a system to have a device with:
- Vulkan 1.3.0<=
- Hardware Accelerated Ray Tracing

**tldr**, if you have a dedicated GPU with hardware ray tracing, you'll most likely be fine.

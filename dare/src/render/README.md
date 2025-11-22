# Rendering Architecture
---
We're currently using a bindless rasterizer architecture.


## Dual worlds
---
The render world and engine world are both ECS systems, however, operate in 2 entirely different worlds that run concurrently of each other.

## Metadata
---
- See: [`src/asset/mod.rs`]
- Responsible for containing information about any asset (buffer, texture, sampler, etc.)
- Metadata is stored seperately in storage and are stored behind opaque handles
- Is meant for multi-threaded concurrent access until concurrent data structures such as dashmap to allow for concurrent queries into the asset map

### Why?
---
- Allows us to have cheap and fast to copy opaque handles to metadata behind our assets

## Resource storage [`src/render2/physical_resource`]
- Stores actual resources and hands out opaque virtual handles
- Allows for allocation of arbitrary virtual handles and binding of subsequent physical resource to back
- Is meant for single-threaded uses only as the render world should only access

### Why?
---
- Provides a virtual resource system to seamlessly upload and handle resources behind an opaque handle



## Uploading
---
- Uploading to the GPU is simple, assets primarily rely on the [`src/asset/loaders/traits.rs`] to provide us uploadable versions of our assets

## TODO
---
- [ ] Create a more coherent pipeline borrowing from Bevy's Extract -> Prepare -> Queue workflow (we do have primitive forms here)



## Extract, Prepare, Queue (TODO)
---
All assets from creation as simply metadata to being loaded into GPU memory follow the extract->prepare->queue pipeline

### Extract
---
- Extraction is responsible for copying over only necessary data 
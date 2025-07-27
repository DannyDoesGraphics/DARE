slangc solid.slang -profile glsl_460 -target spirv -force-glsl-scalar-layout -capability GL_EXT_buffer_reference -emit-spirv-directly -entry vertex_main -o ./compiled/solid.vert.spv
slangc solid.slang -profile glsl_460 -target spirv -force-glsl-scalar-layout -capability GL_EXT_buffer_reference -emit-spirv-directly -entry fragment_main -o ./compiled/solid.frag.spv

slangc cull.slang -profile glsl_460 -target spirv -force-glsl-scalar-layout -capability GL_EXT_buffer_reference -emit-spirv-directly -entry main -o ./compiled/cull.comp.spv
slangc mesh.slang -profile glsl_460 -target spirv -force-glsl-scalar-layout -capability GL_EXT_buffer_reference -emit-spirv-directly -entry vertex_main -o ./compiled/mesh.vert.spv
slangc mesh.slang -profile glsl_460 -target spirv -force-glsl-scalar-layout -capability GL_EXT_buffer_reference -emit-spirv-directly -entry fragment_main -o ./compiled/mesh.frag.spv

slangc solid.slang -profile glsl_460 -target spirv -force-glsl-scalar-layout -capability GL_EXT_buffer_reference -emit-spirv-directly -entry vertex_main -o ./compiled/solid.vert.spv
slangc solid.slang -profile glsl_460 -target spirv -force-glsl-scalar-layout -capability GL_EXT_buffer_reference -emit-spirv-directly -entry fragment_main -o ./compiled/solid.frag.spv
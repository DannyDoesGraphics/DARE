// We provide generic definitions to help keep shader code more "rusty"

#define f32 float
#define f64 double
#define i32 int
#define u32 uint
#ifdef GL_EXT_shader_explicit_arithmetic_types_int64
#define u64 uint64_t
#define i64 int64_t
#endif


// Prevent null errors
#define DIVIDE_OR(a,b,c) ((b) == 0.0 ? (c) : (a) / (b))
#define DIVIDE_OR_ZERO(a,b) DIVIDE_OR(a,b, 0.0)
use core::{mem, ptr};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Format {
    U8,
    U8x3,
    U8x4,
    U16,
    U32,
    U64,
    F32,
    F64,
    F32x2,
    F32x3,
    F32x4,
    F64x2,
    F64x3,
    F64x4,
}

impl Format {
    pub fn size_in_bytes(&self) -> usize {
        match self {
            Format::U8 => 1,
            Format::U8x3 => 3,
            Format::U8x4 => 4,
            Format::U16 => 2,
            Format::U32 => 4,
            Format::U64 => 8,
            Format::F32 => 4,
            Format::F64 => 8,
            Format::F32x2 => 8,
            Format::F32x3 => 12,
            Format::F32x4 => 16,
            Format::F64x2 => 16,
            Format::F64x3 => 24,
            Format::F64x4 => 32,
        }
    }

    pub fn components(&self) -> usize {
        match self {
            Format::U8 | Format::U16 | Format::U32 | Format::U64 | Format::F32 | Format::F64 => 1,
            Format::F32x2 | Format::F64x2 => 2,
            Format::U8x3 | Format::F32x3 | Format::F64x3 => 3,
            Format::U8x4 | Format::F32x4 | Format::F64x4 => 4,
        }
    }

    pub fn signed(&self) -> bool {
        match self {
            Format::U8 | Format::U8x3 | Format::U8x4 | Format::U16 | Format::U32 | Format::U64 => {
                false
            }
            _ => true,
        }
    }
}

// ---------- scalar io (unaligned) ----------

#[inline(always)]
fn read_unaligned<T: Copy>(src: &[u8], off: usize) -> T {
    debug_assert!(off + mem::size_of::<T>() <= src.len());
    unsafe { ptr::read_unaligned(src.as_ptr().add(off) as *const T) }
}

#[inline(always)]
fn write_unaligned<T: Copy>(dst: &mut [u8], off: usize, v: T) {
    debug_assert!(off + mem::size_of::<T>() <= dst.len());
    unsafe { ptr::write_unaligned(dst.as_mut_ptr().add(off) as *mut T, v) }
}

#[inline(always)]
fn f64_to_u8_round_sat(x: f64) -> u8 {
    if x.is_nan() {
        return 0;
    }
    let r = x.round();
    if r <= 0.0 {
        0
    } else if r >= (u8::MAX as f64) {
        u8::MAX
    } else {
        r as u8
    }
}

#[inline(always)]
fn f64_to_u16_round_sat(x: f64) -> u16 {
    if x.is_nan() {
        return 0;
    }
    let r = x.round();
    if r <= 0.0 {
        0
    } else if r >= (u16::MAX as f64) {
        u16::MAX
    } else {
        r as u16
    }
}

#[inline(always)]
fn f64_to_u32_round_sat(x: f64) -> u32 {
    if x.is_nan() {
        return 0;
    }
    let r = x.round();
    if r <= 0.0 {
        0
    } else if r >= (u32::MAX as f64) {
        u32::MAX
    } else {
        r as u32
    }
}

#[inline(always)]
fn f64_to_u64_round_sat(x: f64) -> u64 {
    if x.is_nan() {
        return 0;
    }
    let r = x.round();
    if r <= 0.0 {
        0
    } else if r >= (u64::MAX as f64) {
        u64::MAX
    } else {
        r as u64
    }
}

#[inline(always)]
fn u16_to_u8_sat(x: u16) -> u8 {
    if x > u8::MAX as u16 { u8::MAX } else { x as u8 }
}
#[inline(always)]
fn u32_to_u8_sat(x: u32) -> u8 {
    if x > u8::MAX as u32 { u8::MAX } else { x as u8 }
}
#[inline(always)]
fn u64_to_u8_sat(x: u64) -> u8 {
    if x > u8::MAX as u64 { u8::MAX } else { x as u8 }
}

#[inline(always)]
fn u32_to_u16_sat(x: u32) -> u16 {
    if x > u16::MAX as u32 {
        u16::MAX
    } else {
        x as u16
    }
}
#[inline(always)]
fn u64_to_u16_sat(x: u64) -> u16 {
    if x > u16::MAX as u64 {
        u16::MAX
    } else {
        x as u16
    }
}

#[inline(always)]
fn u64_to_u32_sat(x: u64) -> u32 {
    if x > u32::MAX as u64 {
        u32::MAX
    } else {
        x as u32
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Base {
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
}

#[inline(always)]
fn base(fmt: Format) -> Base {
    match fmt {
        Format::U8 | Format::U8x3 | Format::U8x4 => Base::U8,
        Format::U16 => Base::U16,
        Format::U32 => Base::U32,
        Format::U64 => Base::U64,
        Format::F32 | Format::F32x2 | Format::F32x3 | Format::F32x4 => Base::F32,
        Format::F64 | Format::F64x2 | Format::F64x3 | Format::F64x4 => Base::F64,
    }
}

#[inline(always)]
fn scalar_bytes(b: Base) -> usize {
    match b {
        Base::U8 => 1,
        Base::U16 => 2,
        Base::U32 => 4,
        Base::U64 => 8,
        Base::F32 => 4,
        Base::F64 => 8,
    }
}

// Core kernel: convert one scalar at (in_off) to (out_off).
// This is “exhaustive per pair” but still centralized.
#[inline(always)]
fn convert_one(src: &[u8], in_off: usize, in_b: Base, dst: &mut [u8], out_off: usize, out_b: Base) {
    match (in_b, out_b) {
        (Base::U8, Base::U8) => {
            write_unaligned::<u8>(dst, out_off, read_unaligned::<u8>(src, in_off))
        }
        (Base::U8, Base::U16) => {
            write_unaligned::<u16>(dst, out_off, read_unaligned::<u8>(src, in_off) as u16)
        }
        (Base::U8, Base::U32) => {
            write_unaligned::<u32>(dst, out_off, read_unaligned::<u8>(src, in_off) as u32)
        }
        (Base::U8, Base::U64) => {
            write_unaligned::<u64>(dst, out_off, read_unaligned::<u8>(src, in_off) as u64)
        }
        (Base::U8, Base::F32) => {
            write_unaligned::<f32>(dst, out_off, read_unaligned::<u8>(src, in_off) as f32)
        }
        (Base::U8, Base::F64) => {
            write_unaligned::<f64>(dst, out_off, read_unaligned::<u8>(src, in_off) as f64)
        }

        (Base::U16, Base::U8) => write_unaligned::<u8>(
            dst,
            out_off,
            u16_to_u8_sat(read_unaligned::<u16>(src, in_off)),
        ),
        (Base::U16, Base::U16) => {
            write_unaligned::<u16>(dst, out_off, read_unaligned::<u16>(src, in_off))
        }
        (Base::U16, Base::U32) => {
            write_unaligned::<u32>(dst, out_off, read_unaligned::<u16>(src, in_off) as u32)
        }
        (Base::U16, Base::U64) => {
            write_unaligned::<u64>(dst, out_off, read_unaligned::<u16>(src, in_off) as u64)
        }
        (Base::U16, Base::F32) => {
            write_unaligned::<f32>(dst, out_off, read_unaligned::<u16>(src, in_off) as f32)
        }
        (Base::U16, Base::F64) => {
            write_unaligned::<f64>(dst, out_off, read_unaligned::<u16>(src, in_off) as f64)
        }

        (Base::U32, Base::U8) => write_unaligned::<u8>(
            dst,
            out_off,
            u32_to_u8_sat(read_unaligned::<u32>(src, in_off)),
        ),
        (Base::U32, Base::U16) => write_unaligned::<u16>(
            dst,
            out_off,
            u32_to_u16_sat(read_unaligned::<u32>(src, in_off)),
        ),
        (Base::U32, Base::U32) => {
            write_unaligned::<u32>(dst, out_off, read_unaligned::<u32>(src, in_off))
        }
        (Base::U32, Base::U64) => {
            write_unaligned::<u64>(dst, out_off, read_unaligned::<u32>(src, in_off) as u64)
        }
        (Base::U32, Base::F32) => {
            write_unaligned::<f32>(dst, out_off, read_unaligned::<u32>(src, in_off) as f32)
        }
        (Base::U32, Base::F64) => {
            write_unaligned::<f64>(dst, out_off, read_unaligned::<u32>(src, in_off) as f64)
        }

        (Base::U64, Base::U8) => write_unaligned::<u8>(
            dst,
            out_off,
            u64_to_u8_sat(read_unaligned::<u64>(src, in_off)),
        ),
        (Base::U64, Base::U16) => write_unaligned::<u16>(
            dst,
            out_off,
            u64_to_u16_sat(read_unaligned::<u64>(src, in_off)),
        ),
        (Base::U64, Base::U32) => write_unaligned::<u32>(
            dst,
            out_off,
            u64_to_u32_sat(read_unaligned::<u64>(src, in_off)),
        ),
        (Base::U64, Base::U64) => {
            write_unaligned::<u64>(dst, out_off, read_unaligned::<u64>(src, in_off))
        }
        (Base::U64, Base::F32) => {
            write_unaligned::<f32>(dst, out_off, read_unaligned::<u64>(src, in_off) as f32)
        }
        (Base::U64, Base::F64) => {
            write_unaligned::<f64>(dst, out_off, read_unaligned::<u64>(src, in_off) as f64)
        }

        (Base::F32, Base::U8) => write_unaligned::<u8>(
            dst,
            out_off,
            f64_to_u8_round_sat(read_unaligned::<f32>(src, in_off) as f64),
        ),
        (Base::F32, Base::U16) => write_unaligned::<u16>(
            dst,
            out_off,
            f64_to_u16_round_sat(read_unaligned::<f32>(src, in_off) as f64),
        ),
        (Base::F32, Base::U32) => write_unaligned::<u32>(
            dst,
            out_off,
            f64_to_u32_round_sat(read_unaligned::<f32>(src, in_off) as f64),
        ),
        (Base::F32, Base::U64) => write_unaligned::<u64>(
            dst,
            out_off,
            f64_to_u64_round_sat(read_unaligned::<f32>(src, in_off) as f64),
        ),
        (Base::F32, Base::F32) => {
            write_unaligned::<f32>(dst, out_off, read_unaligned::<f32>(src, in_off))
        }
        (Base::F32, Base::F64) => {
            write_unaligned::<f64>(dst, out_off, read_unaligned::<f32>(src, in_off) as f64)
        }

        (Base::F64, Base::U8) => write_unaligned::<u8>(
            dst,
            out_off,
            f64_to_u8_round_sat(read_unaligned::<f64>(src, in_off)),
        ),
        (Base::F64, Base::U16) => write_unaligned::<u16>(
            dst,
            out_off,
            f64_to_u16_round_sat(read_unaligned::<f64>(src, in_off)),
        ),
        (Base::F64, Base::U32) => write_unaligned::<u32>(
            dst,
            out_off,
            f64_to_u32_round_sat(read_unaligned::<f64>(src, in_off)),
        ),
        (Base::F64, Base::U64) => write_unaligned::<u64>(
            dst,
            out_off,
            f64_to_u64_round_sat(read_unaligned::<f64>(src, in_off)),
        ),
        (Base::F64, Base::F32) => {
            write_unaligned::<f32>(dst, out_off, read_unaligned::<f64>(src, in_off) as f32)
        }
        (Base::F64, Base::F64) => {
            write_unaligned::<f64>(dst, out_off, read_unaligned::<f64>(src, in_off))
        }
    }
}

pub(crate) fn format_convert<T: AsRef<[u8]>>(
    in_format: Format,
    out_format: Format,
    bytes: T,
) -> Vec<u8> {
    let src = bytes.as_ref();
    let in_stride = in_format.size_in_bytes();
    let out_stride = out_format.size_in_bytes();

    assert!(src.len() % in_stride == 0, "Input not tightly packed");
    let elem_count = src.len() / in_stride;

    let in_comp = in_format.components();
    let out_comp = out_format.components();

    let in_base = base(in_format);
    let out_base = base(out_format);

    let in_scalar = scalar_bytes(in_base);
    let out_scalar = scalar_bytes(out_base);

    // sanity: stride should match scalar_bytes * components for all your formats
    debug_assert_eq!(in_stride, in_scalar * in_comp);
    debug_assert_eq!(out_stride, out_scalar * out_comp);

    let mut out = vec![0u8; elem_count * out_stride];

    for e in 0..elem_count {
        let in_elem_off = e * in_stride;
        let out_elem_off = e * out_stride;

        let common = core::cmp::min(in_comp, out_comp);

        // convert shared components
        for c in 0..common {
            let in_off = in_elem_off + c * in_scalar;
            let out_off = out_elem_off + c * out_scalar;
            convert_one(src, in_off, in_base, &mut out, out_off, out_base);
        }
    }

    out
}


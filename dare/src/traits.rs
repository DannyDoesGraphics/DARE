pub trait ReprC {
    type CType: Copy + Clone + Sized;
    fn as_c(&self) -> Self::CType;
}

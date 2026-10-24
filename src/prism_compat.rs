pub(crate) trait LocationExt {
    fn start_offset(&self) -> usize;
    fn end_offset(&self) -> usize;
}

impl LocationExt for ruby_prism::Location<'_> {
    #[expect(
        clippy::as_conversions,
        reason = "Prism offsets are u32 and Tyda targets 32-bit or wider platforms"
    )]
    fn start_offset(&self) -> usize {
        self.start() as usize
    }

    #[expect(
        clippy::as_conversions,
        reason = "Prism offsets are u32 and Tyda targets 32-bit or wider platforms"
    )]
    fn end_offset(&self) -> usize {
        self.end() as usize
    }
}

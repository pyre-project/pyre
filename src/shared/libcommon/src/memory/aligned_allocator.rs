use core::alloc::Allocator;

pub struct AlignedAllocator<const ALIGN: usize, A: Allocator>(pub A);

unsafe impl<const ALIGN: usize, A: Allocator> Allocator for AlignedAllocator<ALIGN, A> {
    fn allocate(&self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
        match layout.align_to(ALIGN) {
            Ok(layout) => self.0.allocate(layout),
            Err(_) => Err(core::alloc::AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        match layout.align_to(ALIGN) {
            Ok(layout) => self.0.deallocate(ptr, layout),
            Err(_) => alloc::alloc::handle_alloc_error(layout),
        }
    }
}

/// Provides a type alias around the default global allocator, always providing page-aligned allocations.
pub fn page_aligned_allocator() -> AlignedAllocator<0x1000, alloc::alloc::Global> {
    AlignedAllocator::<0x1000, _>(alloc::alloc::Global)
}

pub fn stack_aligned_allocator() -> AlignedAllocator<0x10, alloc::alloc::Global> {
    AlignedAllocator::<0x10, _>(alloc::alloc::Global)
}

/// Simple type alias for a page-aligned `Box<T>`.
pub type PageAlignedBox<T> = alloc::boxed::Box<T, AlignedAllocator<0x1000, alloc::alloc::Global>>;

pub type StackAlignedBox<T> = alloc::boxed::Box<T, AlignedAllocator<0x10, alloc::alloc::Global>>;
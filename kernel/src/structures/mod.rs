pub mod gdt;
pub mod tss;
pub mod pic;

pub struct DescriptorTablePointer {
    // size of the DT
    limit: u16,
    // pointer to the memory region containing the DT
    base: u64
}

impl DescriptorTablePointer {
    pub fn limit(self) -> u16 {
self.limit
    }

    pub fn base(self) -> u64 {
        self.base
    }
}
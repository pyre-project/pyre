use crate::{
    interrupts,
    memory::{AttributeModify, FrameManager, Level4, PageAttributes, PageTable, PageTableEntry},
};
use libkernel::{memory::Page, Address, Physical, Virtual};

#[derive(Debug, Clone, Copy)]
pub enum MapError {
    NotMapped,
    AlreadyMapped,
    FrameError(crate::memory::FrameError),
}

struct VirtualMapper {
    mapped_page: Page,
    root_frame_index: usize,
}

impl VirtualMapper {
    /// Attempts to create a new `PageManager`, with `mapped_page` specifying the current page
    /// where the entirety of the system physical memory is mapped.
    ///
    /// SAFETY: This method is unsafe because `mapped_page` can be any value; that is, not necessarily
    ///         a valid address in which physical memory is already mapped. The expectation is that `mapped_page`
    ///         is a proper starting page for the current physical memory mapping.
    pub unsafe fn new(mapped_page: &Page, pml4_index: usize) -> Self {
        Self {
            // We don't know where physical memory is mapped at this point,
            // so rely on what the caller specifies for us.
            mapped_page: *mapped_page,
            root_frame_index: pml4_index,
        }
    }

    fn mapped_offset(&self) -> Address<Virtual> {
        self.mapped_page.address()
    }

    /* ACQUIRE STATE */

    fn pml4_page(&self) -> Page {
        self.mapped_page.forward_checked(self.root_frame_index).unwrap()
    }

    fn pml4(&self) -> Option<&PageTable<Level4>> {
        unsafe { self.pml4_page().address().as_ptr::<PageTable<Level4>>().as_ref() }
    }

    fn pml4_mut(&mut self) -> Option<&mut PageTable<Level4>> {
        unsafe { self.pml4_page().address().as_mut_ptr::<PageTable<Level4>>().as_mut() }
    }

    fn get_page_entry(&self, page: &Page) -> Option<&PageTableEntry> {
        let mapped_page = self.mapped_page;
        let address = page.address();

        unsafe {
            self.pml4()
                .and_then(|p4| p4.sub_table(address.p4_index(), &mapped_page))
                .and_then(|p3| p3.sub_table(address.p3_index(), &mapped_page))
                .and_then(|p2| p2.sub_table(address.p2_index(), &mapped_page))
                .map(|p1| p1.get_entry(address.p1_index()))
        }
    }

    fn get_page_entry_mut(&mut self, page: &Page) -> Option<&mut PageTableEntry> {
        let mapped_page = self.mapped_page;
        let address = page.address();

        unsafe {
            self.pml4_mut()
                .and_then(|p4| p4.sub_table_mut(address.p4_index(), &mapped_page))
                .and_then(|p3| p3.sub_table_mut(address.p3_index(), &mapped_page))
                .and_then(|p2| p2.sub_table_mut(address.p2_index(), &mapped_page))
                .map(|p1| p1.get_entry_mut(address.p1_index()))
        }
    }

    fn get_page_entry_create(&mut self, page: &Page, frame_manager: &'static FrameManager<'_>) -> &mut PageTableEntry {
        let mapped_page = self.mapped_page;
        let address = page.address();

        unsafe {
            self.pml4_mut()
                .unwrap()
                .sub_table_create(address.p4_index(), &mapped_page, frame_manager)
                .sub_table_create(address.p3_index(), &mapped_page, frame_manager)
                .sub_table_create(address.p2_index(), &mapped_page, frame_manager)
                .get_entry_mut(address.p1_index())
        }
    }

    #[inline(always)]
    pub unsafe fn write_root_table(&mut self) {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x64::registers::control::CR3::write(
            Address::<Physical>::new(self.root_frame_index * 0x1000),
            crate::arch::x64::registers::control::CR3Flags::empty(),
        );
    }

    pub fn print_walk(&self, address: Address<Virtual>) {
        let mapped_page = self.mapped_page;

        unsafe {
            #[allow(clippy::bind_instead_of_map)]
            self.pml4()
                .and_then(|table| {
                    info!("L4 {:?}", table.get_entry(address.p4_index()));
                    table.sub_table(address.p4_index(), &mapped_page)
                })
                .and_then(|table| {
                    info!("L3 {:?}", table.get_entry(address.p3_index()));
                    table.sub_table(address.p3_index(), &mapped_page)
                })
                .and_then(|table| {
                    info!("L2 {:?}", table.get_entry(address.p2_index()));
                    table.sub_table(address.p2_index(), &mapped_page)
                })
                .and_then(|table| {
                    info!("L1 {:?}", table.get_entry(address.p1_index()));
                    Some(table.get_entry(address.p1_index()))
                });
        }
    }
}

pub struct PageManager {
    virtual_map: spin::RwLock<VirtualMapper>,
}

unsafe impl Send for PageManager {}
unsafe impl Sync for PageManager {}

impl PageManager {
    /// SAFETY: Refer to `VirtualMapper::new()`.
    pub unsafe fn new(
        frame_manager: &'static FrameManager<'_>,
        mapped_page: &Page,
        pml4_copy: Option<PageTable<Level4>>,
    ) -> Self {
        Self {
            virtual_map: spin::RwLock::new({
                let root_index = frame_manager.lock_next().expect("Failed to lock frame for virtual addressor's PML4");
                let pml4_mapped = mapped_page.forward_checked(root_index).unwrap();

                match pml4_copy {
                    Some(pml4_copy) => pml4_mapped.address().as_mut_ptr::<PageTable<Level4>>().write(pml4_copy),
                    None => core::ptr::write_bytes(pml4_mapped.address().as_mut_ptr::<u8>(), 0, 0x1000),
                }

                VirtualMapper::new(mapped_page, root_index)
            }),
        }
    }

    pub fn root_frame_index(&self) -> usize {
        self.virtual_map.read().root_frame_index
    }

    pub unsafe fn from_current(mapped_page: &Page) -> Self {
        Self {
            virtual_map: spin::RwLock::new(VirtualMapper::new(mapped_page, {
                #[cfg(target_arch = "x86_64")]
                {
                    crate::arch::x64::registers::control::CR3::read().0.frame_index()
                }

                #[cfg(target_arch = "riscv64")]
                {
                    crate::arch::rv64::registers::satp::get_ppn()
                }
            })),
        }
    }

    /* MAP / UNMAP */

    /// Maps the specified page to the specified frame index.
    pub fn map(
        &self,
        page: &Page,
        frame_index: usize,
        lock_frame: bool,
        attributes: PageAttributes,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), MapError> {
        interrupts::without(|| {
            // Lock the virtual map first. This avoids a situation where the frame for this page is
            // freed, an interrupt occurs, and then the page is memory referenced (and thus, a page
            // pointing to a frame it doesn't own is accessed).
            let mut map_write = self.virtual_map.write();

            if page.index() < 10000 {
                info!("{:?} -> {:#X}    :{:?}", page, frame_index, attributes);
            }

            // Attempt to acquire the requisite frame, following the outlined parsing of `lock_frame`.
            let frame_result = if lock_frame { frame_manager.lock(frame_index) } else { Ok(frame_index) };

            match frame_result {
                // If acquisition of the frame is successful, map the page to the frame index.
                Ok(frame_index) => {
                    let entry = map_write.get_page_entry_create(page, frame_manager);
                    entry.set_frame_index(frame_index);
                    entry.set_attributes(attributes, AttributeModify::Set);

                    #[cfg(target_arch = "x86_64")]
                    crate::arch::x64::instructions::tlb::invlpg(page);

                    Ok(())
                }

                // If the acquisition of the frame fails, return the error.
                Err(err) => Err(MapError::FrameError(err)),
            }
        })
    }

    /// Helper function to map MMIO pages and update `FrameManager` state.
    ///
    /// SAFETY: This function trusts implicitly that the provided page and frame index are valid for mapping.
    pub unsafe fn map_mmio(
        &self,
        page: Page,
        frame_index: usize,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), MapError> {
        frame_manager.lock(frame_index).ok();
        frame_manager.force_modify_type(frame_index, crate::memory::FrameType::MMIO).ok();

        if self.is_mapped(page) {
            self.set_page_attributes(&page, PageAttributes::MMIO, AttributeModify::Set);

            Ok(())
        } else {
            self.map(&page, frame_index, false, PageAttributes::MMIO, frame_manager)
        }
    }

    /// Unmaps the given page, optionally freeing the frame the page points to within the given [`FrameManager`].
    pub fn unmap(
        &self,
        page: &Page,
        free_frame: bool,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), MapError> {
        interrupts::without(|| {
            self.virtual_map
                .write()
                .get_page_entry_mut(page)
                .map(|entry| {
                    entry.set_attributes(PageAttributes::VALID, AttributeModify::Remove);

                    // Handle frame permissions to keep them updated.
                    if free_frame {
                        unsafe { frame_manager.free(entry.take_frame_index()).unwrap() };
                    }

                    // Invalidate the page in the TLB.
                    #[cfg(target_arch = "x86_64")]
                    crate::arch::x64::instructions::tlb::invlpg(page);
                })
                .ok_or(MapError::NotMapped)
        })
    }

    pub fn copy_by_map(
        &self,
        unmap_from: &Page,
        map_to: &Page,
        new_attribs: Option<PageAttributes>,
        frame_manager: &'static FrameManager<'_>,
    ) -> Result<(), MapError> {
        interrupts::without(|| {
            let mut map_write = self.virtual_map.write();

            let maybe_new_pte_frame_index_attribs = map_write.get_page_entry_mut(unmap_from).map(|entry| {
                // Get attributes from old frame if none are provided.
                let attribs = new_attribs.unwrap_or_else(|| entry.get_attributes());
                entry.set_attributes(PageAttributes::empty(), AttributeModify::Set);

                (unsafe { entry.take_frame_index() }, attribs)
            });

            maybe_new_pte_frame_index_attribs
                .map(|(new_pte_frame_index, new_pte_attribs)| {
                    // Create the new page table entry with the old entry's data.
                    let entry = map_write.get_page_entry_create(map_to, frame_manager);
                    entry.set_frame_index(new_pte_frame_index);
                    entry.set_attributes(new_pte_attribs, AttributeModify::Set);

                    // Invalidate both old and new pages in TLB.
                    #[cfg(target_arch = "x86_64")]
                    {
                        use crate::arch::x64::instructions::tlb::invlpg;
                        invlpg(map_to);
                        invlpg(unmap_from);
                    }
                })
                .ok_or(MapError::NotMapped)
        })
    }

    pub fn auto_map(&self, page: &Page, attribs: PageAttributes, frame_manager: &'static FrameManager<'_>) {
        self.map(page, frame_manager.lock_next().unwrap(), false, attribs, frame_manager).unwrap();
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, page: Page) -> bool {
        interrupts::without(|| {
            self.virtual_map
                .read()
                .get_page_entry(&page)
                .filter(|entry| entry.get_attributes().contains(PageAttributes::VALID))
                .is_some()
        })
    }

    pub fn is_mapped_to(&self, page: &Page, frame_index: usize) -> bool {
        interrupts::without(|| {
            self.virtual_map.read().get_page_entry(page).map_or(false, |entry| frame_index == entry.get_frame_index())
        })
    }

    pub fn get_mapped_to(&self, page: &Page) -> Option<usize> {
        interrupts::without(|| self.virtual_map.read().get_page_entry(page).map(|entry| entry.get_frame_index()))
    }

    /* STATE CHANGING */

    pub fn get_page_attributes(&self, page: &Page) -> Option<PageAttributes> {
        interrupts::without(|| {
            self.virtual_map.read().get_page_entry(page).map(|page_entry| page_entry.get_attributes())
        })
    }

    pub unsafe fn set_page_attributes(&self, page: &Page, attributes: PageAttributes, modify_mode: AttributeModify) {
        interrupts::without(|| {
            if let Some(page_entry) = self.virtual_map.write().get_page_entry_mut(page) {
                page_entry.set_attributes(attributes, modify_mode);

                #[cfg(target_arch = "x86_64")]
                crate::arch::x64::instructions::tlb::invlpg(page);
            }
        });
    }

    pub fn mapped_page(&self) -> Page {
        interrupts::without(|| self.virtual_map.read().mapped_page)
    }

    #[inline(always)]
    pub unsafe fn write_cr3(&self) {
        interrupts::without(|| {
            self.virtual_map.write().write_root_table();
        });
    }

    pub fn copy_pml4(&self) -> PageTable<Level4> {
        interrupts::without(|| {
            let vmap = self.virtual_map.read();

            unsafe {
                vmap.mapped_page
                    .forward_checked(vmap.root_frame_index)
                    .unwrap()
                    .address()
                    .as_ptr::<PageTable<Level4>>()
                    .read_volatile()
            }
        })
    }

    pub fn print_walk(&self, address: Address<Virtual>) {
        interrupts::without(|| {
            self.virtual_map.read().print_walk(address);
        });
    }

    pub fn print_pml4(&self) {
        interrupts::without(|| {
            let virtual_map = self.virtual_map.read();
            debug!("{:?}", virtual_map.pml4());
        });
    }
}

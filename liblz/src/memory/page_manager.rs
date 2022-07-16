use crate::{
    instructions::tlb,
    memory::{
        frame_manager::FrameOwnership,
        global_fmgr,
        paging::{AttributeModify, Level4, PageAttributes, PageTable, PageTableEntry},
        Page,
    },
    Address, {Physical, Virtual},
};

#[derive(Debug, Clone, Copy)]
pub enum MapError {
    NotMapped,
    AlreadyMapped,
    FrameError(crate::memory::FrameError),
}

struct VirtualMapper {
    mapped_page: Page,
    pml4_frame: usize,
}

impl VirtualMapper {
    /// Attempts to create a new PageManager, with `mapped_page` specifying the current page
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
            pml4_frame: pml4_index,
        }
    }

    fn mapped_offset(&self) -> Address<Virtual> {
        self.mapped_page.base_addr()
    }

    /* ACQUIRE STATE */

    fn pml4_page(&self) -> Page {
        self.mapped_page.forward_checked(self.pml4_frame).unwrap()
    }

    fn pml4(&self) -> Option<&PageTable<Level4>> {
        unsafe { self.pml4_page().as_ptr::<PageTable<Level4>>().as_ref() }
    }

    fn pml4_mut(&mut self) -> Option<&mut PageTable<Level4>> {
        unsafe { self.pml4_page().as_mut_ptr::<PageTable<Level4>>().as_mut() }
    }

    fn get_page_entry(&self, page: &Page) -> Option<&PageTableEntry> {
        let mapped_page = self.mapped_page;
        let addr = page.base_addr();

        unsafe {
            self.pml4()
                .and_then(|p4| p4.sub_table(addr.p4_index(), &mapped_page))
                .and_then(|p3| p3.sub_table(addr.p3_index(), &mapped_page))
                .and_then(|p2| p2.sub_table(addr.p2_index(), &mapped_page))
                .and_then(|p1| Some(p1.get_entry(addr.p1_index())))
        }
    }

    fn get_page_entry_mut(&mut self, page: &Page) -> Option<&mut PageTableEntry> {
        let mapped_page = self.mapped_page;
        let addr = page.base_addr();

        unsafe {
            self.pml4_mut()
                .and_then(|p4| p4.sub_table_mut(addr.p4_index(), &mapped_page))
                .and_then(|p3| p3.sub_table_mut(addr.p3_index(), &mapped_page))
                .and_then(|p2| p2.sub_table_mut(addr.p2_index(), &mapped_page))
                .and_then(|p1| Some(p1.get_entry_mut(addr.p1_index())))
        }
    }

    fn get_page_entry_create(&mut self, page: &Page) -> &mut PageTableEntry {
        let mapped_page = self.mapped_page;
        let addr = page.base_addr();

        // TODO abstract this into the source of page manager

        unsafe {
            self.pml4_mut()
                .unwrap()
                .sub_table_create(addr.p4_index(), &mapped_page)
                .sub_table_create(addr.p3_index(), &mapped_page)
                .sub_table_create(addr.p2_index(), &mapped_page)
                .get_entry_mut(addr.p1_index())
        }
    }

    unsafe fn modify_mapped_page(&mut self, base_page: Page) {
        for frame_index in 0..global_fmgr().total_frames() {
            let cur_page = base_page.forward_checked(frame_index).unwrap();

            self.get_page_entry_create(&cur_page).set(
                frame_index,
                PageAttributes::PRESENT
                    | PageAttributes::WRITABLE
                    | PageAttributes::WRITE_THROUGH
                    | PageAttributes::NO_EXECUTE
                    | PageAttributes::GLOBAL,
            );

            tlb::invalidate(&cur_page);
        }

        self.mapped_page = base_page;
    }

    /// Returns `true` if the current CR3 address matches the addressor's PML4 frame, and `false` otherwise.
    fn is_active(&self) -> bool {
        crate::registers::control::CR3::read().0.frame_index() == self.pml4_frame
    }

    #[inline(always)]
    pub unsafe fn write_cr3(&mut self) {
        crate::registers::control::CR3::write(
            Address::<Physical>::new(self.pml4_frame * 0x1000),
            crate::registers::control::CR3Flags::empty(),
        );
    }

    // TODO
    // fn copy_physical_memory_entries(page_manager: &mut PageManager) {
    //     let k_vmap = PAGE_MANAGER.0.read();
    //     let mut cur_vmap = page_manager.0.write();

    //     let k_pml4 = k_vmap
    //         .pml4()
    //         .expect("Kernel page manager has no valid PML4.");
    //     let cur_pml4 = cur_vmap
    //         .pml4_mut()
    //         .expect("Provided page manager has no valid PML4.");

    //     for entry_index in k_vmap.mapped_offset().p4_index()..512 {
    //         *cur_pml4.get_entry_mut(entry_index) = *k_pml4.get_entry(entry_index);
    //     }
    // }
}

pub struct PageManager(spin::RwLock<VirtualMapper>);

unsafe impl Send for PageManager {}
unsafe impl Sync for PageManager {}

impl PageManager {
    pub fn cr3(&self) -> usize {
        self.0.read().pml4_frame * 0x1000
    }

    /// SAFETY: Refer to `VirtualMapper::new()`.
    pub unsafe fn new(mapped_page: &Page, pml4_copy: Option<PageTable<Level4>>) -> Self {
        Self(spin::RwLock::new({
            let pml4_index = global_fmgr()
                .lock_next()
                .expect("Failed to lock frame for virtual addressor's PML4");

            if let Some(pml4) = pml4_copy {
                // Write existing PML4.
                mapped_page
                    .forward_checked(pml4_index)
                    .unwrap()
                    .as_mut_ptr::<PageTable<Level4>>()
                    .write_volatile(pml4);
            } else {
                // Clear PML4 frame.
                core::ptr::write_bytes(
                    mapped_page
                        .forward_checked(pml4_index)
                        .unwrap()
                        .as_mut_ptr::<u8>(),
                    0,
                    0x1000,
                );
            }

            VirtualMapper::new(mapped_page, pml4_index)
        }))
    }

    pub unsafe fn from_current(mapped_page: &Page) -> Self {
        Self(spin::RwLock::new(VirtualMapper::new(
            mapped_page,
            crate::registers::control::CR3::read().0.frame_index(),
        )))
    }

    /* MAP / UNMAP */

    /// Maps the specified frame to the specified frame index.
    ///
    /// The `lock_frame` parameter is set as follows:
    ///     `Some` or `None` indicates whether frame allocation is required,
    ///         `true` or `false` in a `Some` indicates whether the frame is locked or merely referenced.
    pub fn map(
        &self,
        page: &Page,
        frame_index: usize,
        ownership: FrameOwnership,
        attribs: PageAttributes,
    ) -> Result<(), MapError> {
        // Attempt to acquire the requisite frame, following the outlined parsing of `lock_frame`.
        match ownership {
            FrameOwnership::None => Ok(frame_index),
            FrameOwnership::Borrowed => global_fmgr().borrow(frame_index),
            FrameOwnership::Locked => global_fmgr().lock(frame_index),
        }
        // If the acquisition of the frame fails, transform the error.
        .map_err(|falloc_err| MapError::FrameError(falloc_err))
        // If acquisition of the frame is successful, map the page to the frame index.
        .map(|frame_index| {
            self.0
                .write()
                .get_page_entry_create(page)
                .set(frame_index, attribs);

            tlb::invalidate(page);
        })
    }

    pub fn unmap(&self, page: &Page, ownership: FrameOwnership) -> Result<(), MapError> {
        self.0
            .write()
            .get_page_entry_mut(page)
            .map(|entry| {
                entry.set_attributes(PageAttributes::PRESENT, AttributeModify::Remove);

                // Handle frame permissions to keep them updated.
                unsafe {
                    match ownership {
                        FrameOwnership::None => {}
                        FrameOwnership::Borrowed => {
                            global_fmgr().drop(entry.take_frame_index()).unwrap()
                        }
                        FrameOwnership::Locked => {
                            global_fmgr().free(entry.take_frame_index()).unwrap()
                        }
                    }
                }

                // Invalidate the page in the TLB.
                tlb::invalidate(page);
            })
            .ok_or(MapError::NotMapped)
    }

    pub fn copy_by_map(
        &self,
        unmap_from: &Page,
        map_to: &Page,
        new_attribs: Option<PageAttributes>,
    ) -> Result<(), MapError> {
        let maybe_attribs_frame_index = {
            let mut map_write = self.0.write();
            map_write.get_page_entry_mut(unmap_from).map(|entry| {
                let attribs = new_attribs.unwrap_or_else(|| entry.get_attribs());
                entry.set_attributes(PageAttributes::empty(), AttributeModify::Set);

                unsafe { (attribs, entry.take_frame_index()) }
            })
        };

        maybe_attribs_frame_index
            .ok_or(MapError::NotMapped)
            .map(|(attribs, frame_index)| {
                self.map(map_to, frame_index, FrameOwnership::None, attribs).unwrap();
                tlb::invalidate(unmap_from);

                ()
            })
    }

    pub fn auto_map(&self, page: &Page, attribs: PageAttributes) {
        self.map(page, global_fmgr().lock_next().unwrap(), FrameOwnership::None, attribs)
            .unwrap();
    }

    /// Attempts to create a 1:1 mapping between a virual memory page and its physical counterpart.
    ///
    /// REMARK:
    ///     This function assumes the frame for the identity mapping is:
    ///         A) a valid lock or borrow
    ///         B) not required for the mapping
    pub fn identity_map(&self, page: &Page, attribs: PageAttributes) -> Result<(), MapError> {
        self.map(page, page.index(), FrameOwnership::None, attribs)
    }

    /* STATE QUERYING */

    pub fn is_mapped(&self, virt_addr: Address<Virtual>) -> bool {
        self.0
            .read()
            .get_page_entry(&Page::containing_addr(virt_addr))
            .filter(|entry| entry.get_attribs().contains(PageAttributes::PRESENT))
            .is_some()
    }

    pub fn is_mapped_to(&self, page: &Page, frame_index: usize) -> bool {
        self.0
            .read()
            .get_page_entry(page)
            .and_then(|entry| entry.get_frame_index())
            .map_or(false, |entry_frame_index| frame_index == entry_frame_index)
    }

    pub fn get_mapped_to(&self, page: &Page) -> Option<usize> {
        self.0
            .read()
            .get_page_entry(page)
            .and_then(|entry| entry.get_frame_index())
    }

    /* STATE CHANGING */

    pub fn get_page_attribs(&self, page: &Page) -> Option<PageAttributes> {
        self.0
            .read()
            .get_page_entry(page)
            .map(|page_entry| page_entry.get_attribs())
    }

    pub unsafe fn set_page_attribs(
        &self,
        page: &Page,
        mut attributes: PageAttributes,
        modify_mode: AttributeModify,
    ) {
        if !crate::registers::msr::IA32_EFER::get_nxe() {
            // This bit is reserved if the above bit in IA32_EFER is not set.
            // For now, this means silently removing it for compatability.
            attributes.remove(PageAttributes::NO_EXECUTE);
        }

        self.0
            .write()
            .get_page_entry_mut(page)
            .map(|page_entry| page_entry.set_attributes(attributes, modify_mode));

        tlb::invalidate(page);
    }

    pub unsafe fn modify_mapped_page(&self, page: Page) {
        self.0.write().modify_mapped_page(page);
    }

    pub fn mapped_page(&self) -> Page {
        self.0.read().mapped_page
    }

    #[inline(always)]
    pub unsafe fn write_cr3(&self) {
        self.0.write().write_cr3();
    }

    pub fn copy_pml4(&self) -> PageTable<Level4> {
        let vmap = self.0.read();

        unsafe {
            vmap.mapped_page
                .forward_checked(vmap.pml4_frame)
                .unwrap()
                .as_ptr::<PageTable<Level4>>()
                .read_volatile()
        }
    }
}
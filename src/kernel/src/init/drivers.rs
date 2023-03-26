use limine::{LimineFile, LiminePtr, NonNullPtr};

pub fn load_drivers() {
    let drivers_data = crate::boot::get_kernel_modules()
        // Find module with a valid string name equal to "drivers".
        .and_then(|modules| {
            modules.iter().find(|module| {
                module.path.to_str().and_then(|cstr| cstr.to_str().ok()).filter(|name| name.eq(&"drivers")).is_some()
            })
        })
        // Safety: Bootloader promises the pointer and length to be a valid memory region so long as bootloader memory is unreclaimed.
        .map(|drivers_module| unsafe {
            core::slice::from_raw_parts(drivers_module.base.as_ptr().unwrap(), drivers_module.length as usize)
        })
        .expect("no drivers provided");

    for archive_entry in tar_no_std::TarArchiveRef::new(drivers_data).entries() {
        use crate::memory::{PageAttributes, PageDepth};
        use libsys::{page_shift, page_size};

        debug!("Processing archive entry for driver: {}", archive_entry.filename());

        let Ok(driver_elf) = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(archive_entry.data())
        else {
            warn!("failed to parse driver blob into valid ELF.");
            continue
        };

        // Create the driver's page manager from the kernel's higher-half table.
        // Safety: Kernel guarantees HHDM to be valid.
        let mut driver_mapper = unsafe {
            crate::memory::address_space::Mapper::new_unsafe(
                PageDepth::new(4),
                crate::memory::new_kmapped_page_table().unwrap(),
            )
        };

        let Some(driver_elf_segments) = driver_elf.segments() else { continue };

        // Iterate the segments, and allocate them.
        for segment in driver_elf_segments {
            trace!("{:?}", segment);

            match segment.p_type {
                0x1 => {
                    let memory_size = segment.p_memsz as usize;
                    let memory_start = segment.p_vaddr as usize;
                    let memory_end = memory_start + memory_size;

                    // Align the start address to ensure we iterate page-aligned addresses.
                    let memory_start_aligned = libsys::align_down(memory_start, page_shift());
                    for page_base in (memory_start_aligned..memory_end).step_by(page_size().get()) {
                        use bit_field::BitField;

                        let page = Address::new(page_base).unwrap();
                        // Auto map the virtual address to a physical page.
                        driver_mapper
                            .auto_map(page, {
                                // This doesn't support RWX pages. I'm not sure it ever should.
                                if segment.p_flags.get_bit(1) {
                                    PageAttributes::RX
                                } else if segment.p_flags.get_bit(2) {
                                    PageAttributes::RW
                                } else {
                                    PageAttributes::RO
                                }
                            })
                            .unwrap();
                    }

                    let segment_slice = driver_elf.segment_data(&segment).expect("driver segment parse failure");
                    // Safety: `memory_start` pointer is valid as we just mapped all of the requisite pages for `memory_size` length.
                    let memory_slice = unsafe { core::slice::from_raw_parts_mut(memory_start as *mut u8, memory_size) };
                    // Copy segment data into the new memory region.
                    memory_slice[..segment_slice.len()].copy_from_slice(segment_slice);
                    // Clear any left over bytes to 0. This is useful for the bss region, for example.
                    (&mut memory_slice[segment_slice.len()..]).fill(0x0);
                }

                _ => {}
            }
        }
    }
}

fn with_kernel_module<T>(name: &str, with_fn: impl FnMut(&mut T) -> T) -> Option<T> {
    let kernel_modules = crate::boot::get_kernel_modules().expect("boot memory has been reclaimed");

    for module in kernel_modules.iter() {
        let Some(module_name) = get_module_name(module) else { continue };
        if !module_name.eq("drivers") {
            continue;
        };

        
    }

    let drivers_data = crate::boot::get_kernel_modules()
        // Find module with a valid string name equal to "drivers".
        .and_then(|modules| {
            modules.iter().find(|module| {
                module.path.to_str().and_then(|cstr| cstr.to_str().ok()).filter(|name| name.eq(&"drivers")).is_some()
            })
        })
        // Safety: Bootloader promises the pointer and length to be a valid memory region so long as bootloader memory is unreclaimed.
        .map(|drivers_module| unsafe {
            core::slice::from_raw_parts(drivers_module.base.as_ptr().unwrap(), drivers_module.length as usize)
        })
        .expect("no drivers provided");
}

fn get_module_name<'a>(module: &'a NonNullPtr<LimineFile>) -> Option<&'a str> {
    module.path.to_str().and_then(|cstr| cstr.to_str().ok())
}
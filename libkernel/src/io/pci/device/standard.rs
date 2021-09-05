use crate::{
    addr_ty::Physical,
    io::pci::{PCIeDevice, Standard},
    memory::mmio::{Mapped, MMIO},
    Address,
};
use core::fmt;

#[repr(usize)]
pub enum StandardRegister {
    Reg0 = 0,
    Reg1 = 1,
    Reg2 = 2,
    Reg3 = 3,
    Reg4 = 4,
    Reg5 = 5,
}

impl PCIeDevice<Standard> {
    pub unsafe fn new(mmio: MMIO<Mapped>) -> Self {
        assert_eq!(
            (mmio
                .read::<u8>(crate::io::pci::PCIHeaderOffset::HeaderType.into())
                .unwrap()
                .read())
                & !(1 << 7),
            0,
            "incorrect header type for standard specification PCI device"
        );

        let mut bar_mmios = [None, None, None, None, None, None, None, None, None, None];

        for register_num in 0..=5 {
            let register_raw = mmio
                .read::<u32>(0x10 + (register_num * core::mem::size_of::<u32>()))
                .unwrap()
                .read();

            if register_raw > 0x0 {
                let is_memory_space = (register_raw & 0b1) > 0;
                let addr = Address::<Physical>::new({
                    if is_memory_space {
                        register_raw & !0b1111
                    } else {
                        register_raw & !0b11
                    }
                } as usize);

                trace!(
                    "Device Register {}:\n Raw 0b{:b}\n Canonical: {:?}",
                    register_num,
                    register_raw,
                    addr
                );

                let mmio_frames = crate::memory::falloc::get()
                    .acquire_frames(
                        addr.frame_index(),
                        1,
                        crate::memory::falloc::FrameState::MMIO,
                    )
                    .expect("frames are not MMIO");
                let register_mmio = crate::memory::mmio::unmapped_mmio(mmio_frames)
                    .expect("failed to create MMIO object")
                    .automap();

                if is_memory_space && ((register_raw & 0b1000) > 0) {
                    use crate::memory::paging::{PageAttributeModifyMode, PageAttributes};

                    // optimize page attributes to enable write-through if it wasn't previously enabled
                    crate::memory::malloc::get().modify_page_attributes(
                        &crate::memory::Page::from_addr(register_mmio.mapped_addr()),
                        PageAttributes::WRITE_THROUGH,
                        PageAttributeModifyMode::Insert,
                    )
                }

                bar_mmios[register_num].insert(core::cell::RefCell::new(register_mmio));
            }
        }

        Self {
            mmio,
            bar_mmios,
            phantom: core::marker::PhantomData,
        }
    }

    pub fn get_register(
        &self,
        register: StandardRegister,
    ) -> Option<core::cell::RefMut<MMIO<Mapped>>> {
        self.bar_mmios[register as usize]
            .as_ref()
            .map(|cell| cell.borrow_mut())
    }

    pub fn cardbus_cis_ptr(&self) -> u32 {
        unsafe { self.mmio.read(0x28).unwrap().read() }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { self.mmio.read(0x2C).unwrap().read() }
    }

    pub fn subsystem_id(&self) -> u16 {
        unsafe { self.mmio.read(0x2E).unwrap().read() }
    }

    pub fn expansion_rom_base_addr(&self) -> u32 {
        unsafe { self.mmio.read(0x30).unwrap().read() }
    }

    pub fn capabilities_ptr(&self) -> u8 {
        unsafe { self.mmio.read::<u8>(0x34).unwrap().read() & !0b11 }
    }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { self.mmio.read(0x3C).unwrap().read() } {
            0xFF => None,
            value => Some(value),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { self.mmio.read(0x3D).unwrap().read() } {
            0x0 => None,
            value => Some(value),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { self.mmio.read(0x3E).unwrap().read() }
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { self.mmio.read(0x3F).unwrap().read() }
    }
}

impl fmt::Debug for PCIeDevice<Standard> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let debug_struct = &mut formatter.debug_struct("PCIe Device (Standard)");

        self.generic_debut_fmt(debug_struct);
        debug_struct
            .field(
                "Base Address Register 0",
                &self.get_register(StandardRegister::Reg0),
            )
            .field(
                "Base Address Register 1",
                &self.get_register(StandardRegister::Reg1),
            )
            .field(
                "Base Address Register 2",
                &self.get_register(StandardRegister::Reg2),
            )
            .field(
                "Base Address Register 3",
                &self.get_register(StandardRegister::Reg3),
            )
            .field(
                "Base Address Register 4",
                &self.get_register(StandardRegister::Reg4),
            )
            .field(
                "Base Address Register 5",
                &self.get_register(StandardRegister::Reg5),
            )
            .field("Cardbus CIS Pointer", &self.cardbus_cis_ptr())
            .field("Subsystem Vendor ID", &self.subsystem_vendor_id())
            .field("Subsystem ID", &self.subsystem_id())
            .field(
                "Expansion ROM Base Address",
                &self.expansion_rom_base_addr(),
            )
            .field("Capabilities Pointer", &self.capabilities_ptr())
            .field("Interrupt Line", &self.interrupt_line())
            .field("Interrupt Pin", &self.interrupt_pin())
            .field("Min Grant", &self.min_grant())
            .field("Max Latency", &self.max_latency())
            .finish()
    }
}

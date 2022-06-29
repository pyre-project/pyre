mod capabilities;
pub use capabilities::*;

use crate::{
    io::pci::{DeviceRegister, DeviceRegisterIterator, DeviceType, PCIeDevice, Standard},
    memory::MMIO,
};
use core::fmt;
use num_enum::TryFromPrimitive;

#[repr(usize)]
#[derive(Debug, TryFromPrimitive)]
pub enum StandardRegister {
    Register0 = 0,
    Register1 = 1,
    Register2 = 2,
    Register3 = 3,
    Register4 = 4,
    Register5 = 5,
}

impl PCIeDevice<Standard> {
    pub unsafe fn new(device_mmio: MMIO) -> Self {
        assert_eq!(
            (device_mmio
                .read::<u8>(crate::io::pci::HeaderOffset::HeaderType.into())
                .assume_init())
                & !(1 << 7),
            0,
            "incorrect header type for standard specification PCI device"
        );

        let mut register_num = 0;
        let mut registers = alloc::vec![None, None, None, None, None, None];
        for register in DeviceRegisterIterator::new(
            (device_mmio.mapped_addr() + 0x10).as_mut_ptr::<u32>(),
            Standard::REGISTER_COUNT,
        ) {
            if !register.is_unused() {
                trace!("Device Register {}: {:?}", register_num, register);

                // The address is MMIO, so is memory-mapped—thus, the page index and frame index will match.
                let frame_index = register.as_addr().page_index();
                let frame_usage = crate::align_up_div(register.memory_usage(), 0x1000);
                trace!(
                    "\tAcquiring register destination frame as MMIO: {}:{}",
                    frame_index,
                    frame_usage
                );

                let register_mmio = crate::memory::MMIO::new(frame_index, frame_usage);

                if match register {
                    DeviceRegister::MemorySpace32(value, _) => (value & 0b1000) > 0,
                    DeviceRegister::MemorySpace64(value, _) => (value & 0b1000) > 0,
                    _ => false,
                } {
                    trace!("\tRegister is prefetchable; setting WRITE_THROUGH on MMIO page.");

                    let page_manager = crate::memory::global_pgmr();
                    for page in register_mmio.pages() {
                        use crate::memory::paging::{AttributeModify, PageAttributes};

                        page_manager.set_page_attribs(
                            &page,
                            PageAttributes::PRESENT
                                | PageAttributes::WRITABLE
                                | PageAttributes::WRITE_THROUGH
                                | PageAttributes::UNCACHEABLE
                                | PageAttributes::NO_EXECUTE,
                            AttributeModify::Set,
                        );
                    }
                }

                registers[register_num] = Some(register_mmio);
            }

            match register {
                DeviceRegister::MemorySpace64(_, _) => register_num += 2,
                _ => register_num += 1,
            }
        }

        Self {
            mmio: device_mmio,
            registers,
            phantom: core::marker::PhantomData,
        }
    }

    pub fn cardbus_cis_ptr(&self) -> u32 {
        unsafe { self.mmio.read(0x28).assume_init() }
    }

    pub fn subsystem_vendor_id(&self) -> u16 {
        unsafe { self.mmio.read(0x2C).assume_init() }
    }

    pub fn subsystem_id(&self) -> u16 {
        unsafe { self.mmio.read(0x2E).assume_init() }
    }

    pub fn expansion_rom_base_addr(&self) -> u32 {
        unsafe { self.mmio.read(0x30).assume_init() }
    }

    pub(self) fn capabilities(&self) -> CapablitiesIterator {
        CapablitiesIterator::new(&self.mmio, unsafe {
            (self.mmio.read::<u8>(0x34).assume_init() & !0b11) as usize
        })
    }

    pub fn interrupt_line(&self) -> Option<u8> {
        match unsafe { self.mmio.read(0x3C).assume_init() } {
            0xFF => None,
            value => Some(value),
        }
    }

    pub fn interrupt_pin(&self) -> Option<u8> {
        match unsafe { self.mmio.read(0x3D).assume_init() } {
            0x0 => None,
            value => Some(value),
        }
    }

    pub fn min_grant(&self) -> u8 {
        unsafe { self.mmio.read(0x3E).assume_init() }
    }

    pub fn max_latency(&self) -> u8 {
        unsafe { self.mmio.read(0x3F).assume_init() }
    }

    pub fn get_register(&self, register: StandardRegister) -> Option<&MMIO> {
        self.registers[register as usize].as_ref()
    }

    pub fn iter_registers(&self) -> core::slice::Iter<Option<MMIO>> {
        self.registers.iter()
    }

    pub fn find_msix(&self) -> Option<MSIX> {
        MSIX::try_new(self)
    }
}

impl fmt::Debug for PCIeDevice<Standard> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let debug_struct = &mut formatter.debug_struct("PCIe Device (Standard)");

        self.generic_debut_fmt(debug_struct);
        debug_struct
            .field("Cardbus CIS Pointer", &self.cardbus_cis_ptr())
            .field("Subsystem Vendor ID", &self.subsystem_vendor_id())
            .field("Subsystem ID", &self.subsystem_id())
            .field(
                "Expansion ROM Base Address",
                &self.expansion_rom_base_addr(),
            )
            .field("Interrupt Line", &self.interrupt_line())
            .field("Interrupt Pin", &self.interrupt_pin())
            .field("Min Grant", &self.min_grant())
            .field("Max Latency", &self.max_latency())
            .finish()
    }
}
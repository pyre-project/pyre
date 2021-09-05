mod command;
mod port;

pub use command::*;
pub use port::*;

use libkernel::{volatile::VolatileCell, ReadOnly};

#[repr(C)]
pub struct HBAMemory {
    host_capability: VolatileCell<u32, ReadOnly>,
    global_host_control: VolatileCell<u32, ReadOnly>,
    interrupt_status: VolatileCell<u32, ReadOnly>,
    ports_implemented: VolatileCell<u32, ReadOnly>,
    version: VolatileCell<u32, ReadOnly>,
    ccc_control: VolatileCell<u32, ReadOnly>,
    ccc_ports: VolatileCell<u32, ReadOnly>,
    enclosure_management_location: VolatileCell<u32, ReadOnly>,
    enclosure_management_control: VolatileCell<u32, ReadOnly>,
    host_capabilities_extended: VolatileCell<u32, ReadOnly>,
    bios_handoff_control_status: VolatileCell<u32, ReadOnly>,
    _reserved0: [u8; 0x74],
    _vendor0: [u8; 0x60],
    ports: [HBAPort; 32],
}

impl HBAMemory {
    fn ports_implemented(&self) -> usize {
        let mut bits = 0;
        let mut bit = 1;

        let ports_impletemented = self.ports_implemented.read();
        while (ports_impletemented & bit) > 0 {
            bits += 1;
            bit <<= 1;
        }

        bits
    }

    pub fn ports(&self) -> core::slice::Iter<HBAPort> {
        self.ports[0..self.ports_implemented()].iter()
    }

    pub fn ports_mut(&mut self) -> core::slice::IterMut<HBAPort> {
        let ports_implemented = self.ports_implemented();
        self.ports[0..ports_implemented].iter_mut()
    }
}

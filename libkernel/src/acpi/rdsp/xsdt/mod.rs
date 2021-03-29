pub mod madt;
pub mod mcfg;

use crate::acpi::{ACPITable, Checksum, SDTHeader, SizedACPITable};

#[derive(Debug)]
pub enum XSDTError {
    NoXSDT,
    NoEntry,
}

pub trait XSDTEntryType {
    const SIGNATURE: &'static str;
}

pub struct XSDTEntry<T: XSDTEntryType> {
    phantom: core::marker::PhantomData<T>,
}

impl<T: XSDTEntryType> XSDTEntry<T> {
    pub fn sdt_header(&self) -> &SDTHeader {
        unsafe { &*(self as *const _ as *const _) }
    }
}

impl<T: XSDTEntryType> Checksum for XSDTEntry<T> {
    fn bytes_len(&self) -> usize {
        self.sdt_header().table_len() as usize
    }
}

impl<T: XSDTEntryType> ACPITable for XSDTEntry<T> {
    fn body_len(&self) -> usize {
        self.sdt_header().table_len() as usize
    }
}

#[repr(C)]
pub struct XSDT<'entry> {
    header: SDTHeader,
    phantom: core::marker::PhantomData<&'entry u8>,
}

impl<'entry> XSDT<'entry> {
    pub fn header(&self) -> &SDTHeader {
        &self.header
    }

    pub fn get_entry<T: XSDTEntryType>(&self) -> Result<&'entry XSDTEntry<T>, XSDTError> {
        for entry_ptr in self.entries().iter().map(|entry_ptr| *entry_ptr) {
            unsafe {
                if (&*(entry_ptr as *const _ as *const SDTHeader)).signature() == T::SIGNATURE {
                    let entry: &XSDTEntry<T> = &*(entry_ptr as *const _ as *const _);
                    entry.checksum_panic();
                    return Ok(entry);
                }
            }
        }

        Err(XSDTError::NoEntry)
    }
}

impl ACPITable for XSDT<'_> {
    fn body_len(&self) -> usize {
        (self.header().table_len() as usize) - core::mem::size_of::<SDTHeader>()
    }
}

impl SizedACPITable<SDTHeader, *const u64> for XSDT<'_> {}

impl Checksum for XSDT<'_> {
    fn bytes_len(&self) -> usize {
        self.header().table_len() as usize
    }
}

lazy_static::lazy_static! {
    pub static ref LAZY_XSDT: Option<&'static XSDT<'static>> = unsafe {
        crate::acpi::rdsp::LAZY_RDSP2.map(|rdsp2| {
            let xsdt = &*(rdsp2.xsdt_addr().as_usize() as *const XSDT<'static>);
            xsdt.checksum_panic();
            xsdt
        })
    };
}
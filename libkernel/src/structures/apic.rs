use core::marker::PhantomData;

use crate::{registers::MSR, structures::GUID};
use x86_64::PhysAddr;

pub const ACPI_GUID: GUID = GUID::new(
    0xeb9d2d30,
    0x2d88,
    0x11d3,
    0x9a16,
    [0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
);

pub const ACPI2_GUID: GUID = GUID::new(
    0x8868e871,
    0xe4f1,
    0x11d3,
    0xbc22,
    [0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
);

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum InvariantAPICRegister {
    ID = 0x20,
    Version = 0x30,
    TaskPriority = 0x80,
    LDR = 0xD0,
    DFR = 0xE0,
    ESR = 0x280,
    ICRL = 0x300,
    ICRH = 0x310,
    TimerInitialCount = 0x380,
    TimeCurrentCount = 0x390,
    TimerDivisor = 0x3E0,
    Last = 0x38F,
    TimerBaseDivisor = 1 << 20,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICTimerMode {
    OneShot = 0,
    Periodic = 1 << 17,
    TSC_Deadline = 1 << 18,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICTimerDivisor {
    Div2 = 0b0000,
    Div4 = 0b0001,
    Div8 = 0b0010,
    Div16 = 0b0011,
    Div32 = 0b1000,
    Div64 = 0b1001,
    Div128 = 0b1010,
    Div1 = 0b1011,
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum APICDeliveryMode {
    Fixed = 0b000,
    SMI = 0b010,
    NMI = 0b100,
    ExtINT = 0b111,
    INIT = 0b101,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum APICInterruptRegister {
    CMCI,
    Timer,
    ThermalSensor,
    PerformanceCounter,
    LINT0,
    LINT1,
    Error,
}

pub struct APIC {
    base_addr: PhysAddr,
}

impl APIC {
    const LVT_CMCI: u16 = 0x2F0;
    const LVT_TIMER: u16 = 0x320;
    const LVT_THERMAL_SENSOR: u16 = 0x330;
    const LVT_PERFORMANCE: u16 = 0x340;
    const LVT_LINT0: u16 = 0x350;
    const LVT_LINT1: u16 = 0x360;
    const LVT_ERROR: u16 = 0x370;

    pub const LVT_INT_MASKED: u128 = 1 << 16;
    pub const DISABLE: u128 = 0x10000;
    pub const NMI: u128 = 4 << 8;
    pub const SW_ENABLE: u128 = 0x100;
    pub const CPU_FOCUS: u128 = 0x200;

    pub fn from_msr() -> Self {
        Self {
            base_addr: PhysAddr::new(MSR::IA32_APIC_BASE.read() & !(0xFFF)),
        }
    }

    pub unsafe fn from_addr(base_addr: PhysAddr) -> Self {
        Self { base_addr }
    }

    #[inline]
    fn get_register(&self, offset: u16) -> &u128 {
        unsafe { &*((self.base_addr + (offset as u64)).as_u64() as *const u128) }
    }

    #[inline]
    fn get_register_mut(&mut self, offset: u16) -> &mut u128 {
        unsafe { &mut *((self.base_addr + (offset as u64)).as_u64() as *mut u128) }
    }

    #[inline]
    pub fn signal_eoi(&mut self) {
        *self.get_register_mut(0xB0) = 0;
    }

    #[inline]
    pub fn cmci(&mut self) -> APICRegister<Generic> {
        APICRegister::new(self.get_register_mut(Self::LVT_CMCI))
    }

    #[inline]
    pub fn timer(&mut self) -> APICRegister<Timer> {
        APICRegister::new(self.get_register_mut(Self::LVT_TIMER))
    }

    #[inline]
    pub fn lint0(&mut self) -> APICRegister<LINT> {
        APICRegister::new(self.get_register_mut(Self::LVT_LINT0))
    }

    #[inline]
    pub fn lint1(&mut self) -> APICRegister<LINT> {
        APICRegister::new(self.get_register_mut(Self::LVT_LINT1))
    }

    #[inline]
    pub fn error(&mut self) -> APICRegister<Error> {
        APICRegister::new(self.get_register_mut(Self::LVT_ERROR))
    }

    #[inline]
    pub fn performance(&mut self) -> APICRegister<Generic> {
        APICRegister::new(self.get_register_mut(Self::LVT_PERFORMANCE))
    }

    #[inline]
    pub fn thermal_sensor(&mut self) -> APICRegister<Generic> {
        APICRegister::new(self.get_register_mut(Self::LVT_THERMAL_SENSOR))
    }

    #[inline]
    pub fn set_spurious(&mut self, vector: u8) {
        *self.get_register_mut(0xF0) = vector as u128;
    }
}

pub trait APICRegisterVariant {}

pub enum Timer {}
impl APICRegisterVariant for Timer {}

pub enum Generic {}
impl APICRegisterVariant for Generic {}

pub enum LINT {}
impl APICRegisterVariant for LINT {}

pub enum Error {}
impl APICRegisterVariant for Error {}

use bit_field::BitField;

#[repr(transparent)]
pub struct APICRegister<'val, T: APICRegisterVariant + ?Sized> {
    value: &'val mut u128,
    phantom: PhantomData<T>,
}

impl<'val, T: APICRegisterVariant> APICRegister<'val, T> {
    #[inline]
    fn new(value: &'val mut u128) -> Self {
        Self {
            value,
            phantom: PhantomData,
        }
    }

    #[inline]
    pub fn is_interrupted(&self) -> bool {
        self.value.get_bit(12)
    }

    #[inline]
    pub fn is_masked(&self) -> bool {
        self.value.get_bit(12)
    }

    #[inline]
    pub fn set_masked(&mut self, masked: bool) {
        self.value.set_bit(12, masked);
    }

    #[inline]
    pub fn get_vector(&self) -> u8 {
        self.value.get_bits(0..8) as u8
    }

    #[inline]
    pub fn set_vector(&mut self, vector: u8) {
        self.value.set_bits(0..8, vector as u128);
    }
}

impl APICRegister<'_, Timer> {
    #[inline]
    pub fn set_mode(&mut self, mode: APICTimerMode) {
        self.value.set_bits(17..19, mode as u128);
    }
}

impl APICRegister<'_, Generic> {
    #[inline]
    pub fn set_delivery_mode(&mut self, mode: APICDeliveryMode) {
        self.value.set_bits(8..11, mode as u128);
    }
}

impl core::ops::Index<InvariantAPICRegister> for APIC {
    type Output = u128;

    #[inline]
    fn index(&self, register: InvariantAPICRegister) -> &Self::Output {
        self.get_register(register as u16)
    }
}

impl core::ops::IndexMut<InvariantAPICRegister> for APIC {
    #[inline]
    fn index_mut(&mut self, register: InvariantAPICRegister) -> &mut Self::Output {
        self.get_register_mut(register as u16)
    }
}
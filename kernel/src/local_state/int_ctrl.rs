use core::num::NonZeroU32;
use libkernel::structures::apic::APIC;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptVector {
    GlobalTimer = 32,
    LocalTimer = 48,
    CMCI = 49,
    Performance = 50,
    ThermalSensor = 51,
    LINT0 = 52,
    LINT1 = 53,
    Error = 54,
    Storage = 55,
    // APIC spurious interrupt is default mapped to 255.
    Spurious = u8::MAX,
}

pub struct InterruptController {
    apic: APIC,
    per_ms: u32,
}

impl InterruptController {
    pub fn create() -> Self {
        use libkernel::structures::apic::*;

        // Ensure interrupts are enabled.
        libkernel::instructions::interrupts::enable();

        trace!("Configuring APIC & APIT.");
        let apic = unsafe { APIC::from_msr(Some(&*crate::memory::PAGE_MANAGER)) };
        unsafe { apic.reset() };
        libkernel::asm_marker!(0x1FAAA);
        apic.write_register(Register::TimerDivisor, TimerDivisor::Div1 as u32);
        libkernel::asm_marker!(0x1FAAB);
        apic.timer().set_mode(TimerMode::OneShot);
        libkernel::asm_marker!(0x1FAAC);

        let per_10ms = {
            //trace!("Determining APIT frequency.");
            // Wait on the global timer, to ensure we're starting the count on the rising edge of each millisecond.
            crate::clock::global::busy_wait_msec(1);
            // 'Enable' the APIT to begin counting down in `Register::TimerCurrentCount`
            apic.write_register(Register::TimerInitialCount, u32::MAX);
            // Wait for 10ms to get good average tickrate.
            crate::clock::global::busy_wait_msec(10);

            apic.read_register(Register::TimerCurrentCount)
        };
        libkernel::asm_marker!(0x1FAAE);

        let per_ms = (u32::MAX - per_10ms) / 10;
        trace!("APIT frequency: {}Hz", per_10ms * 100);

        // Configure timer vector.
        apic.timer().set_vector(InterruptVector::LocalTimer as u8);
        apic.timer().set_masked(false);
        // Configure error vector.
        apic.error().set_vector(InterruptVector::Error as u8);
        apic.error().set_masked(false);
        // Set default vectors.
        // REMARK: Any of these left masked are not currently supported.
        apic.cmci().set_vector(InterruptVector::CMCI as u8);
        apic.performance()
            .set_vector(InterruptVector::Performance as u8);
        apic.thermal_sensor()
            .set_vector(InterruptVector::ThermalSensor as u8);
        apic.lint0().set_vector(InterruptVector::LINT0 as u8);
        apic.lint1().set_vector(InterruptVector::LINT1 as u8);

        trace!("Core-local APIC configured.");

        Self { apic, per_ms }
    }

    #[inline]
    pub fn apic_id(&self) -> u8 {
        self.apic.id()
    }

    #[inline]
    pub unsafe fn sw_enable(&self) {
        self.apic.sw_enable();
    }

    #[inline]
    pub unsafe fn sw_disable(&self) {
        self.apic.sw_disable();
    }

    #[inline]
    pub fn icr(&self) -> libkernel::structures::apic::icr::InterruptCommandRegister {
        self.apic.interrupt_command()
    }

    #[inline]
    pub fn reload_timer(&self, ms_multiplier: Option<NonZeroU32>) {
        self.apic.write_register(
            libkernel::structures::apic::Register::TimerInitialCount,
            ms_multiplier
                .unwrap_or(unsafe { NonZeroU32::new_unchecked(1) })
                .get()
                * self.per_ms,
        );
    }

    #[inline]
    pub(super) fn end_of_interrupt(&self) {
        self.apic.end_of_interrupt();
    }
}

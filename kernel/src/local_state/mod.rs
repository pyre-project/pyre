mod int_ctrl;

use bit_field::BitField;
pub use int_ctrl::*;
use spin::{Mutex, MutexGuard};

use crate::{clock::AtomicClock, scheduling::Scheduler};
use core::sync::atomic::AtomicUsize;
use lib::registers::{msr, msr::GenericMSR};

pub static INIT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn is_bsp() -> bool {
    msr::IA32_APIC_BASE::is_bsp()
}

struct LocalStateRegister;

impl LocalStateRegister {
    const ID_FLAG: u64 = 1 << 0;
    const PTR_FLAG: u64 = 1 << 1;
    const ID_BITS_SHFT: u64 = (Self::PTR_FLAG.trailing_zeros() as u64) + 1;
    const ID_BITS: u64 = 0xFF << Self::ID_BITS_SHFT;
    const DATA_MASK: u64 = Self::ID_BITS | Self::PTR_FLAG | Self::ID_FLAG;

    #[inline]
    fn get_id() -> u8 {
        let gs_base = msr::IA32_GS_BASE::read();
        if (gs_base & Self::ID_FLAG) == 0 {
            let cpuid_id = (lib::instructions::cpuid::exec(0x1, 0x0).unwrap().ebx() >> 24) as u64;

            unsafe {
                msr::IA32_GS_BASE::write(
                    msr::IA32_GS_BASE::read() | (cpuid_id << Self::ID_BITS_SHFT) | Self::ID_FLAG,
                )
            };

            cpuid_id as u8
        } else {
            ((gs_base & Self::ID_BITS) >> Self::ID_BITS_SHFT) as u8
        }
    }

    fn try_get() -> Option<&'static LocalState> {
        unsafe {
            let gs_base = msr::IA32_GS_BASE::read();
            if (gs_base & Self::PTR_FLAG) > 0 {
                ((gs_base & !Self::DATA_MASK) as *mut LocalState).as_ref()
            } else {
                None
            }
        }
    }

    fn set_local_state_ptr(ptr: *mut LocalState) {
        let ptr_u64 = ptr as u64;

        assert_eq!(
            ptr_u64 & Self::DATA_MASK,
            0,
            "LocalState pointer cannot have data bits set."
        );

        unsafe {
            msr::IA32_GS_BASE::write(
                ptr_u64 | (msr::IA32_GS_BASE::read() & Self::DATA_MASK) | Self::PTR_FLAG,
            )
        };
    }
}

struct LocalState {
    clock: AtomicClock,
    int_ctrl: InterruptController,
    thread: Mutex<Scheduler>,
}

pub fn init() {
    assert!(
        !LocalStateRegister::try_get().is_some(),
        "Local state register has already been configured."
    );

    INIT_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    let cpuid_id = {
        lib::instructions::cpuid::exec(0x1, 0x0)
            .unwrap()
            .ebx()
            .get_bits(24..) as u8
    };

    trace!("Configuring local state: {}.", cpuid_id);
    unsafe {
        let lpu_ptr = lib::memory::malloc::get()
            .alloc(
                core::mem::size_of::<LocalState>(),
                // Invariantly asssumes LocalStateFlags bitfields will be packed.
                core::num::NonZeroUsize::new((LocalStateRegister::DATA_MASK as usize) + 1),
            )
            .unwrap()
            .cast::<LocalState>()
            .unwrap()
            .into_parts()
            .0;

        {
            let clock = AtomicClock::new();
            let int_ctrl = InterruptController::create();
            let thread = Mutex::new(Scheduler::new());

            assert_eq!(
                cpuid_id,
                int_ctrl.apic_id(),
                "CPUID processor ID does not match APIC ID."
            );

            // Write the LPU structure into memory.
            lpu_ptr.write(LocalState {
                clock,
                int_ctrl,
                thread,
            });
        }

        // Convert ptr to 64 bit representation, and write metadata into low bits.
        LocalStateRegister::set_local_state_ptr(lpu_ptr);
        int_ctrl().sw_enable();
        int_ctrl().reload_timer(core::num::NonZeroU32::new(1));
    }
}

static LOCAL_STATE_NO_INIT: &str = "Processor local state has not been initialized";

pub fn processor_id() -> u8 {
    LocalStateRegister::get_id()
}

pub fn clock() -> &'static AtomicClock {
    LocalStateRegister::try_get()
        .map(|ls| &ls.clock)
        .expect(LOCAL_STATE_NO_INIT)
}

pub fn int_ctrl() -> &'static InterruptController {
    LocalStateRegister::try_get()
        .map(|ls| &ls.int_ctrl)
        .expect(LOCAL_STATE_NO_INIT)
}

pub fn lock_scheduler() -> MutexGuard<'static, Scheduler> {
    LocalStateRegister::try_get()
        .map(|ls| ls.thread.lock())
        .expect(LOCAL_STATE_NO_INIT)
}

pub fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    LocalStateRegister::try_get().and_then(|ls| ls.thread.try_lock())
}
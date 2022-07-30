pub mod cpuid;
pub mod interrupts;
pub mod pwm;
pub mod tlb;

use core::arch::asm;

#[inline(always)]
pub fn pause() {
    unsafe { asm!("pause", options(nostack, nomem, preserves_flags)) };
}

#[inline(always)]
pub fn hlt() {
    unsafe {
        asm!("hlt", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn hlt_indefinite() -> ! {
    loop {
        hlt();
    }
}

#[inline(always)]
pub unsafe fn set_data_registers(value: u16) {
    asm!(
        "mov ds, ax",
        "mov es, ax",
        "mov gs, ax",
        "mov fs, ax",
        "mov ss, ax",
        in("ax") value,
        options(readonly, nostack, preserves_flags)
    );
}

#[derive(Debug, Clone, Copy)]
pub enum RandError {
    NotSupported,
    HardFailure,
}

/// Reads a (hopefully) cryptographically secure, deterministic random number from hardware using the `rdrand` instruction.
pub fn rdrand() -> Result<u64, RandError> {
    // Check to ensure the instruction is supported.
    if crate::cpu::has_feature(crate::cpu::Feature::RDRAND) {
        // In the case of a hard failure for random number generation, a retry limit is employed
        // to stop software from entering a busy loop due to bad `rdrand` values.
        for _ in 0..100 {
            let result: u64;
            let rflags: u64;

            unsafe {
                asm!(
                    "
                    pushfq      # Save original `rflags`
                    rdrand {}
                    pushfq      # Save `rdrand` `rflags`
                    pop {}      # Pop `rflags` into local variable
                    popfq       # Restore original `rflags`
                    ",
                    out(reg) result,
                    out(reg) rflags,
                    options(pure, nomem, preserves_flags)
                );
            }

            // IA32 Software Developer's Manual specifies it is (rarely) possible for `rdrand` to return
            // bad data in the destination register. If this is the case—and additionally if demand for random
            // number generation is too high—the CF bit in `rflags` will not be set, and in the latter case (throughput),
            // zero will be returned in the destination register.
            use crate::registers::RFlags;
            if result > 0 && RFlags::from_bits_truncate(rflags).contains(RFlags::CARRY_FLAG) {
                return Ok(result);
            } else {
                pause();
            }
        }

        Err(RandError::HardFailure)
    } else {
        Err(RandError::NotSupported)
    }
}


/// Reads a (hopefully) cryptographically secure, deterministic random number from hardware using the `rdseed` instruction.
pub fn rdseed() -> Result<u64, RandError> {
        // Check to ensure the instruction is supported.
        if crate::cpu::has_feature(crate::cpu::Feature::RDSEED) {
            // In the case of a hard failure for random number generation, a retry limit is employed
            // to stop software from entering a busy loop due to bad values.
            for _ in 0..100 {
                let result: u64;
                let rflags: u64;
    
                unsafe {
                    asm!(
                        "
                        pushfq      # Save original `rflags`
                        rdseed {}
                        pushfq      # Save `rdrand` `rflags`
                        pop {}      # Pop `rflags` into local variable
                        popfq       # Restore original `rflags`
                        ",
                        out(reg) result,
                        out(reg) rflags,
                        options(pure, nomem, preserves_flags)
                    );
                }
    
                // IA32 Software Developer's Manual specifies it is (rarely) possible for `rdseed` to return
                // bad data in the destination register. If this is the case—and additionally if demand for random
                // number generation is too high—the CF bit in `rflags` will not be set, and in the latter case (throughput),
                // zero will be returned in the destination register.
                use crate::registers::RFlags;
                if result > 0 && RFlags::from_bits_truncate(rflags).contains(RFlags::CARRY_FLAG) {
                    return Ok(result);
                } else {
                    pause();
                }
            }
    
            Err(RandError::HardFailure)
        } else {
            Err(RandError::NotSupported)
        }
}

#[inline]
pub fn mfence() {
    unsafe { core::arch::asm!("mfence", options(nostack, nomem, preserves_flags)) };
}
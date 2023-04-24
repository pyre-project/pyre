#[cfg(target_arch = "x86_64")]
mod x64;
#[cfg(target_arch = "x86_64")]
pub use x64::*;

use crate::proc::{Registers, State};
use libsys::syscall::{Result, Vector};

/// ### Safety
///
/// This function should never be called by software.
#[allow(clippy::similar_names, clippy::no_effect_underscore_binding)]
pub(self) fn sanitize(
    vector: u64,
    arg0: u64,
    arg1: u64,
    _arg2: u64,
    _arg3: u64,
    _arg4: u64,
    state: &mut State,
    regs: &mut Registers,
) -> Result {
    match Vector::try_from(vector) {
        Err(err) => {
            warn!("Unhandled system call vector: {:X?}", err);
            Result::InvalidVector
        }

        Ok(Vector::KlogInfo) => process_klog(log::Level::Info, arg0, arg1),
        Ok(Vector::KlogError) => process_klog(log::Level::Error, arg0, arg1),
        Ok(Vector::KlogDebug) => process_klog(log::Level::Debug, arg0, arg1),
        Ok(Vector::KlogTrace) => process_klog(log::Level::Trace, arg0, arg1),

        Ok(Vector::ProcExit) => crate::local::with_scheduler(|scheduler| {
            info!("Exiting task: {:?}", scheduler.process().map(crate::proc::Process::id));
            scheduler.exit_task(state, regs);

            Result::Ok
        }),
        Ok(Vector::ProcYield) => todo!(),
    }
}

fn process_klog(level: log::Level, str_ptr_arg: u64, str_len_arg: u64) -> Result {
    let str_ptr = usize::try_from(str_ptr_arg).map_err(Result::from)? as *mut u8;
    let str_len = usize::try_from(str_len_arg).map_err(Result::from)?;

    let str = core::str::from_utf8(unsafe { core::slice::from_raw_parts(str_ptr, str_len) }).map_err(Result::from)?;
    log!(level, "[KLOG]: {}", str);

    Result::Ok
}

use crate::uptr;

#[cfg(target_arch = "x86_64")]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Registers {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub rfl: crate::arch::x64::registers::RFlags,
    pub cs: u64,
    pub ss: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub ip: uptr,
    pub sp: uptr,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Context(State, Registers);

impl Context {
    #[inline]
    pub const fn new(state: State, regs: Registers) -> Self {
        Self(state, regs)
    }

    #[inline]
    pub const fn state(&self) -> &State {
        &self.0
    }

    #[inline]
    pub fn state_mut(&mut self) -> &mut State {
        &mut self.0
    }

    #[inline]
    pub const fn regs(&self) -> &Registers {
        &self.1
    }

    #[inline]
    pub fn regs_mut(&mut self) -> &mut Registers {
        &mut self.1
    }
}

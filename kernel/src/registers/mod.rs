#[cfg(target_arch = "x86_64")]
pub mod x64;

#[cfg(target_arch = "riscv64")]
pub mod rv64;

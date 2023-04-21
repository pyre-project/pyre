use num_enum::TryFromPrimitive;

#[repr(C)]
#[derive(Debug)]
pub enum SyscallResult<T> {
    Ok(T),
    Err(Error),
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum Error {
    InvalidPtr = 0x600,
    NonUtf8Str = 0x700,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, Hash)]
pub enum Vector {
    SyslogInfo = 0x100,
    SyslogError = 0x101,
    SyslogDebug = 0x102,
    SyslogTrace = 0x103,
}

pub fn syslog_info(str: &str) -> Result<()> {
    let str_ptr = str.as_ptr();
    let str_len = str.len();
}

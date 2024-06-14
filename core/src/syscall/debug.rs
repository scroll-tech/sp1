use crate::runtime::{Syscall, SyscallContext};

pub struct SyscallDebug;

impl SyscallDebug {
    pub const fn new() -> Self {
        Self
    }
}

impl Syscall for SyscallDebug {
    fn execute(&self, _: &mut SyscallContext, _: u32, _: u32) -> Option<u32> {
        None
    }
}

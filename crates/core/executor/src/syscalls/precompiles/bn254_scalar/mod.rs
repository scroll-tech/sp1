use crate::{
    events::{create_bn254_scalar_arith_event, Bn254FieldOperation, PrecompileEvent},
    syscalls::{Syscall, SyscallCode, SyscallContext},
};

pub(crate) struct Bn254ScalarMacSyscall;

impl Syscall for Bn254ScalarMacSyscall {
    fn execute(
        &self,
        rt: &mut SyscallContext,
        syscall_code: SyscallCode,
        arg1: u32,
        arg2: u32,
    ) -> Option<u32> {
        let event = create_bn254_scalar_arith_event(rt, arg1, arg2, Bn254FieldOperation::Mac);
        rt.record_mut().add_precompile_event(syscall_code, PrecompileEvent::Bn254ScalarMac(event));

        None
    }

    fn num_extra_cycles(&self) -> u32 {
        1
    }
}

pub(crate) struct Bn254ScalarMulSyscall;
impl Syscall for Bn254ScalarMulSyscall {
    fn execute(
        &self,
        rt: &mut SyscallContext,
        syscall_code: SyscallCode,
        arg1: u32,
        arg2: u32,
    ) -> Option<u32> {
        let event = create_bn254_scalar_arith_event(rt, arg1, arg2, Bn254FieldOperation::Mul);
        rt.record_mut().add_precompile_event(syscall_code, PrecompileEvent::Bn254ScalarMul(event));

        None
    }

    fn num_extra_cycles(&self) -> u32 {
        1
    }
}

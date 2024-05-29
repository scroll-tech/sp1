cfg_if::cfg_if! {
    if #[cfg(target_os = "zkvm")] {
        use core::arch::asm;
    }
}

#[no_mangle]
pub extern "C" fn syscall_marker_in(a: usize, b: usize) {
    #[cfg(target_os = "zkvm")]
    unsafe {
        asm!(
            "ecall",
            in("t0") crate::syscalls::MARKER_IN,
            in("a0") a,
            in("a1") b,
        );
    }

    #[cfg(not(target_os = "zkvm"))]
    unreachable!()
}

#[no_mangle]
pub extern "C" fn syscall_marker_out(a: usize, b: usize) {
    #[cfg(target_os = "zkvm")]
    unsafe {
        asm!(
            "ecall",
            in("t0") crate::syscalls::MARKER_OUT,
            in("a0") a,
            in("a1") b,
        );
    }

    #[cfg(not(target_os = "zkvm"))]
    unreachable!()
}

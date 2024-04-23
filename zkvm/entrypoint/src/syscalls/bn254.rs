#[cfg(target_os = "zkvm")]
use core::arch::asm;

/// Adds two Bn254 points.
///
/// The result is stored in the first point.
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn syscall_bn254_add(p: *mut u32, q: *const u32) {
    #[cfg(target_os = "zkvm")]
    unsafe {
        asm!(
            "ecall",
            in("t0") crate::syscalls::BN254_ADD,
            in("a0") p,
            in("a1") q,
        );
    }

    #[cfg(not(target_os = "zkvm"))]
    unreachable!()
}

/// Double a Bn254 point.
///
/// The result is stored in the first point.
#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn syscall_bn254_double(p: *mut u32) {
    #[cfg(target_os = "zkvm")]
    unsafe {
        asm!(
            "ecall",
            in("t0") crate::syscalls::BN254_DOUBLE,
            in("a0") p,
            in("a1") 0,
        );
    }

    #[cfg(not(target_os = "zkvm"))]
    unreachable!()
}

#[allow(unused_variables)]
#[no_mangle]
pub extern "C" fn syscall_bn254_scalar_arith(pq: *mut u32, op: *const u32) {
    #[cfg(target_os = "zkvm")]
    unsafe {
        asm!(
            "ecall",
            in("t0") crate::syscalls::BN254_SCALAR_ARITH,
            in("a0") pq,
            in("a1") op,
        );
    }

    #[cfg(not(target_os = "zkvm"))]
    unreachable!()
}

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
pub extern "C" fn syscall_bn254_scalar_mac(
    ret: *mut u32,
    a: *const u32,
    b: *const u32,
    c: *const u32,
) {
    let q = &[a, b, c];
    let q_ptr = q.as_ptr() as *const u32;
    #[cfg(target_os = "zkvm")]
    unsafe {
        asm!(
            "ecall",
            in("t0") crate::syscalls::BN254_SCALAR_MAC,
            in("a0") ret,
            in("a1") q_ptr,
        );
    }

    #[cfg(not(target_os = "zkvm"))]
    unreachable!()
}

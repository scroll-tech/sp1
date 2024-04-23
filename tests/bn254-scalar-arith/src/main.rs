extern "C" {
    fn syscall_bn254_scalar_arith(pq: *mut u32, op: *const u32);
}

fn main() {
    let mut pq: [u8; 64] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ];
    let op: [u8; 4] = [0, 0, 0, 0];

    let c: [u8; 32] = [
        3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    unsafe {
        syscall_bn254_scalar_arith(pq.as_mut_ptr() as *mut u32, op.as_ptr() as *const u32);
    }

    assert_eq!(&pq[0..32], &c);
}

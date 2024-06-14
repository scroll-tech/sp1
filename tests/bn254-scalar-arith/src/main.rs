#![no_main]
sp1_zkvm::entrypoint!(main);

extern "C" {
    fn syscall_bn254_scalar_mul(p: *mut u32, q: *const u32);
}

fn main() {
    let mut p: [u8; 32] = [
        69, 44, 191, 0, 130, 148, 188, 171, 61, 47, 83, 78, 152, 132, 196, 72, 209, 29, 140, 237,
        126, 75, 223, 58, 115, 139, 235, 236, 200, 47, 2, 28,
    ];
    let q: [u8; 32] = [
        169, 1, 76, 216, 163, 152, 124, 133, 147, 234, 54, 244, 220, 34, 93, 224, 147, 90, 136,
        227, 1, 70, 214, 130, 142, 119, 53, 203, 16, 160, 133, 43,
    ];

    let c: [u8; 32] = [
        43, 132, 167, 109, 73, 175, 44, 161, 152, 82, 20, 126, 173, 132, 9, 50, 112, 242, 217, 141,
        87, 50, 0, 64, 74, 105, 9, 124, 167, 37, 39, 37,
    ];

    // println!("cycle-tracker-start: bn254_scalar_mul");
    unsafe {
        syscall_bn254_scalar_mul(p.as_mut_ptr() as *mut u32, q.as_ptr() as *const u32);
    }
    // println!("cycle-tracker-end: bn254_scalar_mul");

    assert_eq!(p, c);
}

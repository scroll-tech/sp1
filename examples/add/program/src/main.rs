//! A simple program to be proven inside the zkVM.

#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    // NOTE: values of n larger than 186 will overflow the u128 type,
    // resulting in output that doesn't match fibonacci sequence.
    // However, the resulting proof will still be valid!
    let n = 1u32 << 20;
    let mut sum = 0_u32;
    for _ in 1..=n {
        sum += 1;
    }

    // sp1_zkvm::io::commit(&a);
    // sp1_zkvm::io::commit(&b);
}

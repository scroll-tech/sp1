//! A simple program to be proven inside the zkVM.
#![no_main]
sp1_zkvm::entrypoint!(main);

use halo2curves::bn256::Fr;
use halo2curves::ff::PrimeField;
use poseidon_base::Hashable;

pub fn main() {
    let message = [Fr::from(1), Fr::from(2)];

    let mut results = Vec::new();
    for i in 0..20 {
        println!("cycle-tracker-start: hash with domain");
        results.push(Fr::hash_with_domain(message, Fr::zero()));
        println!("cycle-tracker-end: hash with domain");
    }
    // Use the results in some way that the compiler can't optimize away.
    let c = if results.len() > 0 {
        results[0]
    } else {
        Fr::zero()
    };

    let mut c_le_bytes = c.to_repr().to_vec();
    c_le_bytes.reverse();

    assert_eq!(
        hex::encode(&c_le_bytes),
        "115cc0f5e7d690413df64c6b9662e9cf2a3617f2743245519e19607a4417189a"
    );
}

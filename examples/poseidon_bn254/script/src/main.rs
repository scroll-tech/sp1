//! A simple script to generate and verify the proof of a given program.
// use sp1_core::{SP1Prover, SP1Stdin, SP1Verifier};
use sp1_sdk::{utils, ProverClient, SP1Stdin};

const ELF: &[u8] = include_bytes!("../../program/elf/riscv32im-succinct-zkvm-elf");

fn main() {
    utils::setup_tracer();

    // Generate proof.
    let mut stdin = SP1Stdin::new();

    let client = ProverClient::new();
    let mut proof = client.prove(ELF, stdin).expect("proving failed");

    // Verify proof.
    client.verify(ELF, &proof).expect("verification failed");

    // Save proof.
    proof
        .save("proof-with-io.json")
        .expect("saving proof failed");

    println!("successfully generated and verified proof for the program!")
}

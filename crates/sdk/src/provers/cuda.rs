use anyhow::Result;
use sp1_cuda::SP1CudaProver;
use sp1_prover::{components::DefaultProverComponents, SP1Prover, SP1Stdin};

use super::ProverType;
use crate::{
    provers::{try_install_circuit_artifacts, ProofOpts},
    Prover, SP1Context, SP1Proof, SP1ProofKind, SP1ProofWithPublicValues, SP1ProvingKey,
    SP1VerifyingKey,
};

/// An implementation of [crate::ProverClient] that can generate proofs locally using CUDA.
pub struct CudaProver {
    prover: SP1Prover<DefaultProverComponents>,
    cuda_prover: SP1CudaProver,
}

impl CudaProver {
    /// Creates a new [CudaProver].
    pub fn new() -> Self {
        let prover = SP1Prover::new();
        let cuda_prover = SP1CudaProver::new();
        Self { prover, cuda_prover }
    }
}

impl Prover<DefaultProverComponents> for CudaProver {
    fn id(&self) -> ProverType {
        ProverType::Cuda
    }

    fn setup(&self, elf: &[u8]) -> (SP1ProvingKey, SP1VerifyingKey) {
        self.prover.setup(elf)
    }

    fn sp1_prover(&self) -> &SP1Prover<DefaultProverComponents> {
        &self.prover
    }

    fn prove<'a>(
        &'a self,
        pk: &SP1ProvingKey,
        stdin: SP1Stdin,
        opts: ProofOpts,
        _context: SP1Context<'a>,
        kind: SP1ProofKind,
    ) -> Result<SP1ProofWithPublicValues> {
        tracing::warn!("opts and context are ignored for the cuda prover");

        // Generate the core proof.
        let proof = self.cuda_prover.prove_core(pk, &stdin)?;
        if kind == SP1ProofKind::Core {
            return Ok(SP1ProofWithPublicValues {
                proof: SP1Proof::Core(proof.proof.0),
                stdin: proof.stdin,
                public_values: proof.public_values,
                sp1_version: self.version().to_string(),
            });
        }

        let deferred_proofs = stdin.proofs.iter().map(|p| p.0.clone()).collect();
        let public_values = proof.public_values.clone();

        // Generate the compressed proof.
        let reduce_proof = self.cuda_prover.compress(&pk.vk, proof, deferred_proofs)?;
        if kind == SP1ProofKind::Compressed {
            return Ok(SP1ProofWithPublicValues {
                proof: SP1Proof::Compressed(reduce_proof.proof),
                stdin,
                public_values,
                sp1_version: self.version().to_string(),
            });
        }

        // Generate the shrink proof.
        let compress_proof = self.prover.shrink(reduce_proof, opts.sp1_prover_opts)?;

        // Genenerate the wrap proof.
        let outer_proof = self.prover.wrap_bn254(compress_proof, opts.sp1_prover_opts)?;

        let plonk_bn254_aritfacts = if sp1_prover::build::sp1_dev_mode() {
            sp1_prover::build::try_build_plonk_bn254_artifacts_dev(
                self.prover.wrap_vk(),
                &outer_proof.proof,
            )
        } else {
            try_install_circuit_artifacts()
        };
        let proof = self.prover.wrap_plonk_bn254(outer_proof, &plonk_bn254_aritfacts);
        if kind == SP1ProofKind::Plonk {
            return Ok(SP1ProofWithPublicValues {
                proof: SP1Proof::Plonk(proof),
                stdin,
                public_values,
                sp1_version: self.version().to_string(),
            });
        }

        unreachable!()
    }
}

impl Default for CudaProver {
    fn default() -> Self {
        Self::new()
    }
}

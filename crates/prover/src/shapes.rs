use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs::File,
    panic::{catch_unwind, AssertUnwindSafe},
    path::PathBuf,
    sync::{Arc, Mutex},
};

use p3_baby_bear::BabyBear;
use p3_field::AbstractField;
use serde::{Deserialize, Serialize};
use sp1_core_machine::riscv::CoreShapeConfig;
use sp1_recursion_circuit::{
    machine::{
        SP1CompressWithVKeyWitnessValues, SP1CompressWithVkeyShape, SP1DeferredShape,
        SP1DeferredWitnessValues, SP1RecursionShape, SP1RecursionWitnessValues,
    },
    merkle_tree::MerkleTree,
};
use sp1_recursion_core::{shape::RecursionShapeConfig, RecursionProgram};
use sp1_stark::{MachineProver, ProofShape, DIGEST_SIZE};

pub const SHAPE_BYTES: &[u8] = include_bytes!("../shapes.bin");

use crate::{components::SP1ProverComponents, CompressAir, HashableKey, InnerSC, SP1Prover};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SP1ProofShape {
    Recursion(ProofShape),
    Compress(Vec<ProofShape>),
    Deferred(ProofShape),
    Shrink(ProofShape),
}

#[derive(Clone, Serialize, Deserialize)]
pub struct VkData {
    pub vk_map: BTreeMap<[BabyBear; DIGEST_SIZE], usize>,
    pub root: [BabyBear; DIGEST_SIZE],
    pub merkle_tree: MerkleTree<BabyBear, InnerSC>,
}

impl VkData {
    pub fn save(&self, build_dir: PathBuf, dummy: bool) -> Result<(), std::io::Error> {
        let mut file = if dummy {
            File::create(build_dir.join("dummy_vk_data.bin"))?
        } else {
            File::create(build_dir.join("vk_data.bin"))?
        };
        bincode::serialize_into(&mut file, self).unwrap();
        Ok(())
    }

    pub fn load(build_dir: PathBuf) -> Result<Self, std::io::Error> {
        let mut file = File::open(build_dir.join("vk_data.bin"))?;
        let vk_data: Self = bincode::deserialize_from(&mut file).unwrap();
        Ok(vk_data)
    }

    pub fn new(vk_set: BTreeSet<[BabyBear; DIGEST_SIZE]>, height: usize) -> Self {
        let vk_map: BTreeMap<_, _> =
            vk_set.into_iter().enumerate().map(|(i, vk_digest)| (vk_digest, i)).collect();

        // Build a merkle tree from the vk map.
        let mut vks_padded = vk_map.keys().cloned().collect::<Vec<_>>();
        assert!(vks_padded.len() < 1 << height);
        vks_padded.resize(1 << height, <[BabyBear; DIGEST_SIZE]>::default());
        tracing::info!("building merkle tree");
        let (root, merkle_tree) =
            MerkleTree::<BabyBear, InnerSC>::commit(vk_map.keys().cloned().collect());

        VkData { vk_map, root, merkle_tree }
    }
}

#[derive(Debug, Clone)]
pub enum SP1CompressProgramShape {
    Recursion(SP1RecursionShape),
    Compress(SP1CompressWithVkeyShape),
    Deferred(SP1DeferredShape),
    Shrink(SP1CompressWithVkeyShape),
}

pub fn build_vk_map<C: SP1ProverComponents>(
    reduce_batch_size: usize,
    dummy: bool,
    num_compiler_workers: usize,
    num_setup_workers: usize,
    indices: Option<Vec<usize>>,
) -> (BTreeSet<[BabyBear; DIGEST_SIZE]>, Vec<usize>, usize) {
    let mut prover = SP1Prover::<C>::new();
    prover.vk_verification = !dummy;
    let core_shape_config = prover.core_shape_config.as_ref().expect("core shape config not found");
    let recursion_shape_config =
        prover.recursion_shape_config.as_ref().expect("recursion shape config not found");

    tracing::info!("building compress vk map");
    let (vk_set, panic_indices, height) = if dummy {
        tracing::warn!("Making a dummy vk map");
        let dummy_set = SP1ProofShape::dummy_vk_map(
            core_shape_config,
            recursion_shape_config,
            reduce_batch_size,
        )
        .into_keys()
        .collect::<BTreeSet<_>>();
        let height = dummy_set.len().next_power_of_two().ilog2() as usize;
        (dummy_set, vec![], height)
    } else {
        let (vk_tx, vk_rx) = std::sync::mpsc::channel();
        let (shape_tx, shape_rx) =
            std::sync::mpsc::sync_channel::<(usize, SP1CompressProgramShape)>(num_compiler_workers);
        let (program_tx, program_rx) = std::sync::mpsc::sync_channel(num_setup_workers);
        let (panic_tx, panic_rx) = std::sync::mpsc::channel();

        let shape_rx = Mutex::new(shape_rx);
        let program_rx = Mutex::new(program_rx);

        let indices_set = indices.map(|indices| indices.into_iter().collect::<HashSet<_>>());
        let all_shapes =
            SP1ProofShape::generate(core_shape_config, recursion_shape_config, reduce_batch_size)
                .collect::<BTreeSet<_>>();
        let num_shapes = all_shapes.len();

        let height = num_shapes.next_power_of_two().ilog2() as usize;
        let chunk_size = indices_set.as_ref().map(|indices| indices.len()).unwrap_or(num_shapes);

        std::thread::scope(|s| {
            // Initialize compiler workers.
            for _ in 0..num_compiler_workers {
                let program_tx = program_tx.clone();
                let shape_rx = &shape_rx;
                let prover = &prover;
                let panic_tx = panic_tx.clone();
                s.spawn(move || {
                    while let Ok((i, shape)) = shape_rx.lock().unwrap().recv() {
                        println!("shape {} is {:?}", i, shape);
                        let program = catch_unwind(AssertUnwindSafe(|| {
                            prover.program_from_shape(shape.clone())
                        }));
                        match program {
                            Ok(program) => program_tx.send((i, program)).unwrap(),
                            Err(e) => {
                                tracing::warn!(
                                    "Program generation failed for shape {} {:?}, with error: {:?}",
                                    i,
                                    shape,
                                    e
                                );
                                panic_tx.send(i).unwrap();
                            }
                        }
                    }
                });
            }

            // Initialize setup workers.
            for _ in 0..num_setup_workers {
                let vk_tx = vk_tx.clone();
                let program_rx = &program_rx;
                let prover = &prover;
                s.spawn(move || {
                    let mut done = 0;
                    while let Ok((i, program)) = program_rx.lock().unwrap().recv() {
                        let setup_result = tracing::debug_span!("setup for program {}", i)
                            .in_scope(|| {
                                catch_unwind(AssertUnwindSafe(|| {
                                    prover.compress_prover.setup(&program)
                                }))
                            });
                        done += 1;
                        match setup_result {
                            Ok((_, vk)) => {
                                let vk_digest = vk.hash_babybear();
                                tracing::info!(
                                    "program {} = {:?}, {}% done",
                                    i,
                                    vk_digest,
                                    done * 100 / chunk_size
                                );
                                vk_tx.send(vk_digest).unwrap();
                            }
                            Err(e) => {
                                tracing::warn!("setup for program {} failed: {:?}", i, e);
                            }
                        }
                    }
                });
            }

            // Generate shapes and send them to the compiler workers.
            all_shapes
                .into_iter()
                .enumerate()
                .filter(|(i, _)| indices_set.as_ref().map(|set| set.contains(i)).unwrap_or(true))
                .map(|(i, shape)| (i, SP1CompressProgramShape::from_proof_shape(shape, height)))
                .for_each(|(i, program_shape)| {
                    shape_tx.send((i, program_shape)).unwrap();
                });

            drop(shape_tx);
            drop(program_tx);
            drop(vk_tx);
            drop(panic_tx);

            let vk_set = vk_rx.iter().collect::<BTreeSet<_>>();

            let panic_indices = panic_rx.iter().collect::<Vec<_>>();

            (vk_set, panic_indices, height)
        })
    };
    tracing::info!("compress vks generated, number of keys: {}", vk_set.len());
    (vk_set, panic_indices, height)
}

pub fn build_vk_map_to_file<C: SP1ProverComponents>(
    build_dir: PathBuf,
    reduce_batch_size: usize,
    dummy: bool,
    num_compiler_workers: usize,
    num_setup_workers: usize,
    range_start: Option<usize>,
    range_end: Option<usize>,
) {
    std::fs::create_dir_all(&build_dir).expect("failed to create build directory");

    tracing::info!("Building vk set");

    let (vk_set, _, height) = build_vk_map::<C>(
        reduce_batch_size,
        dummy,
        num_compiler_workers,
        num_setup_workers,
        range_start.and_then(|start| range_end.map(|end| (start..end).collect())),
    );

    tracing::info!("Creating vk data from vk set");
    let vk_data = VkData::new(vk_set, height);

    vk_data.save(build_dir, dummy).expect("failed to save vk data");
}

impl SP1ProofShape {
    pub fn generate<'a>(
        core_shape_config: &'a CoreShapeConfig<BabyBear>,
        recursion_shape_config: &'a RecursionShapeConfig<BabyBear, CompressAir<BabyBear>>,
        reduce_batch_size: usize,
    ) -> impl Iterator<Item = Self> + 'a {
        // let shapes: Vec<SP1ProofShape> = bincode::deserialize(SHAPE_BYTES).unwrap();
        // shapes.into_iter().chain(
        //     recursion_shape_config
        //         .get_all_shape_combinations(1)
        //         .map(|mut x| Self::Shrink(x.pop().unwrap())),
        // )
        core_shape_config
            .generate_all_allowed_shapes()
            .map(Self::Recursion)
            .chain(
                recursion_shape_config
                    .get_all_shape_combinations(1)
                    .map(|mut x| Self::Deferred(x.pop().unwrap())),
            )
            .chain(
                recursion_shape_config
                    .get_all_shape_combinations(reduce_batch_size)
                    .map(Self::Compress),
            )
            .chain(
                recursion_shape_config
                    .get_all_shape_combinations(1)
                    .map(|mut x| Self::Shrink(x.pop().unwrap())),
            )
    }

    pub fn dummy_vk_map<'a>(
        core_shape_config: &'a CoreShapeConfig<BabyBear>,
        recursion_shape_config: &'a RecursionShapeConfig<BabyBear, CompressAir<BabyBear>>,
        reduce_batch_size: usize,
    ) -> BTreeMap<[BabyBear; DIGEST_SIZE], usize> {
        Self::generate(core_shape_config, recursion_shape_config, reduce_batch_size)
            .enumerate()
            .map(|(i, _)| ([BabyBear::from_canonical_usize(i); DIGEST_SIZE], i))
            .collect()
    }
}

impl SP1CompressProgramShape {
    pub fn from_proof_shape(shape: SP1ProofShape, height: usize) -> Self {
        match shape {
            SP1ProofShape::Recursion(proof_shape) => Self::Recursion(proof_shape.into()),
            SP1ProofShape::Deferred(proof_shape) => {
                Self::Deferred(SP1DeferredShape::new(vec![proof_shape].into(), height))
            }
            SP1ProofShape::Compress(proof_shapes) => Self::Compress(SP1CompressWithVkeyShape {
                compress_shape: proof_shapes.into(),
                merkle_tree_height: height,
            }),
            SP1ProofShape::Shrink(proof_shape) => Self::Shrink(SP1CompressWithVkeyShape {
                compress_shape: vec![proof_shape].into(),
                merkle_tree_height: height,
            }),
        }
    }
}

impl<C: SP1ProverComponents> SP1Prover<C> {
    pub fn program_from_shape(
        &self,
        shape: SP1CompressProgramShape,
    ) -> Arc<RecursionProgram<BabyBear>> {
        match shape {
            SP1CompressProgramShape::Recursion(shape) => {
                let input = SP1RecursionWitnessValues::dummy(self.core_prover.machine(), &shape);
                self.recursion_program(&input)
            }
            SP1CompressProgramShape::Deferred(shape) => {
                let input = SP1DeferredWitnessValues::dummy(self.compress_prover.machine(), &shape);
                self.deferred_program(&input)
            }
            SP1CompressProgramShape::Compress(shape) => {
                let input =
                    SP1CompressWithVKeyWitnessValues::dummy(self.compress_prover.machine(), &shape);
                self.compress_program(&input)
            }
            SP1CompressProgramShape::Shrink(shape) => {
                let input =
                    SP1CompressWithVKeyWitnessValues::dummy(self.compress_prover.machine(), &shape);
                self.shrink_program(&input)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_all_shapes() {
        let core_shape_config = CoreShapeConfig::default();
        let recursion_shape_config = RecursionShapeConfig::default();
        let reduce_batch_size = 2;
        let all_shapes =
            SP1ProofShape::generate(&core_shape_config, &recursion_shape_config, reduce_batch_size)
                .collect::<BTreeSet<_>>();

        println!("Number of compress shapes: {}", all_shapes.len());

        let test_shapes: BTreeSet<SP1ProofShape> = bincode::deserialize(SHAPE_BYTES).unwrap();
        for shape in test_shapes {
            assert!(all_shapes.contains(&shape), "shape {:?} not found", shape);
        }
    }
}

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use p3_air::Air;
use p3_baby_bear::BabyBear;
use p3_field::extension::BinomialExtensionField;
use p3_matrix::dense::DenseMatrix;
use p3_matrix::Matrix;
use p3_uni_stark::SymbolicAirBuilder;
use rand::distributions::Standard;
use rand::{thread_rng, Rng};
use sp1_core::air::MachineAir;
use sp1_core::alu::{AddSubChip, MulChip};
use sp1_core::cpu::CpuChip;
use sp1_core::io::SP1Stdin;
use sp1_core::lookup::InteractionBuilder;
use sp1_core::runtime::{ExecutionRecord, Program, Runtime};
use sp1_core::stark::Chip;
use sp1_core::utils::{prove, BabyBearPoseidon2, SP1CoreOpts};

type E = BinomialExtensionField<BabyBear, 4>;

#[allow(unreachable_code)]
pub fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("prove");
    group.sample_size(10);
    let programs = ["fibonacci"];
    for p in programs {
        let elf_path = format!("../examples/{}/program/elf/riscv32im-succinct-zkvm-elf", p);
        let program = Program::from_elf(&elf_path);
        let cycles = {
            let mut runtime = Runtime::new(program.clone(), SP1CoreOpts::default());
            runtime.run().unwrap();
            runtime.state.global_clk
        };
        group.bench_function(
            format!("main:{}:{}", p.split('/').last().unwrap(), cycles),
            |b| {
                b.iter(|| {
                    prove(
                        black_box(program.clone()),
                        &SP1Stdin::new(),
                        BabyBearPoseidon2::new(),
                        SP1CoreOpts::default(),
                    )
                })
            },
        );
    }
    group.finish();
}

fn bench_multiset_equality_trace_for_chip<
    A: MachineAir<BabyBear, Record = ExecutionRecord>
        + Air<InteractionBuilder<BabyBear>>
        + Air<SymbolicAirBuilder<BabyBear>>,
>(
    c: &mut Criterion,
    chip: A,
) {
    let mut rng = thread_rng();
    let chip = Chip::new(chip);

    let p = "fibonacci";
    let elf_path = format!("../examples/{}/program/elf/riscv32im-succinct-zkvm-elf", p);
    let program = Program::from_elf(&elf_path);

    let mut group = c.benchmark_group(format!("generate_mse_trace<{}>", chip.name()));
    group.sample_size(10);

    for fib_n in [1 << 10, 1 << 12, 1 << 14, 1 << 16, 1 << 17, 1 << 18] {
        let mut runtime = Runtime::new(program.clone(), SP1CoreOpts::default());
        runtime.write_stdin(&fib_n);
        let cycles = {
            runtime.run().unwrap();
            runtime.state.global_clk
        };
        let main_trace: DenseMatrix<BabyBear> =
            chip.generate_trace(&runtime.record, &mut ExecutionRecord::default());

        group.bench_function(
            format!("cycles={}, trace_height={}", cycles, main_trace.height()),
            |b| {
                b.iter(|| {
                    let alpha_beta: Vec<E> = vec![rng.sample(Standard); 2];
                    chip.generate_permutation_trace(None, &main_trace, &alpha_beta);
                })
            },
        );
    }
}

fn bench_multiset_equality_trace(c: &mut Criterion) {
    bench_multiset_equality_trace_for_chip(c, AddSubChip::default());
    bench_multiset_equality_trace_for_chip(c, CpuChip::default());
    bench_multiset_equality_trace_for_chip(c, MulChip::default());
}

criterion_group!(benches, bench_multiset_equality_trace, criterion_benchmark);
criterion_main!(benches);

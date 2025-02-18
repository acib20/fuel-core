mod contract;
mod utils;
mod vm_set;

use criterion::{
    black_box,
    criterion_group,
    criterion_main,
    measurement::WallTime,
    BenchmarkGroup,
    Criterion,
};

use contract::*;
use fuel_core_benches::*;
use fuel_core_types::fuel_asm::Instruction;
use vm_set::*;

// Use Jemalloc during benchmarks
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

pub fn run_group_ref<I>(group: &mut BenchmarkGroup<WallTime>, id: I, bench: VmBench)
where
    I: AsRef<str>,
{
    let mut i = bench.prepare().expect("failed to prepare bench");
    group.bench_function::<_, _>(id.as_ref(), move |b| {
        b.iter_custom(|iters| {
            let VmBenchPrepared {
                vm,
                instruction,
                diff,
            } = &mut i;
            let checkpoint = vm
                .as_mut()
                .database_mut()
                .checkpoint()
                .expect("Should be able to create a checkpoint");
            let original_db = core::mem::replace(vm.as_mut().database_mut(), checkpoint);

            let final_time;
            loop {
                // Measure the total time to revert the VM to the initial state.
                // It should always do the same things regardless of the number of
                // iterations because we use a `diff` from the `VmBenchPrepared` initialization.
                let start = std::time::Instant::now();
                for _ in 0..iters {
                    vm.reset_vm_state(diff);
                }
                let time_to_reset = start.elapsed();

                let start = std::time::Instant::now();
                for _ in 0..iters {
                    match instruction {
                        Instruction::CALL(call) => {
                            let (ra, rb, rc, rd) = call.unpack();
                            vm.prepare_call(ra, rb, rc, rd).unwrap();
                        }
                        _ => {
                            black_box(vm.instruction(*instruction).unwrap());
                        }
                    }
                    vm.reset_vm_state(diff);
                }
                let only_instruction = start.elapsed().checked_sub(time_to_reset);

                // It may overflow when the benchmarks run in an unstable environment.
                // If the hardware is busy during the measuring time to reset the VM,
                // it will produce `time_to_reset` more than the actual time
                // to run the instruction and reset the VM.
                if let Some(result) = only_instruction {
                    final_time = result;
                    break
                } else {
                    println!("The environment is unstable. Rerunning the benchmark.");
                }
            }

            // restore original db
            *vm.as_mut().database_mut() = original_db;
            final_time
        })
    });
}

fn vm(c: &mut Criterion) {
    alu::run(c);
    blockchain::run(c);
    crypto::run(c);
    flow::run(c);
    mem::run(c);
    contract_root(c);
    state_root(c);
}

criterion_group!(benches, vm);
criterion_main!(benches);

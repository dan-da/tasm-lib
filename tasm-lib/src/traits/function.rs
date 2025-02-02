use std::{cell::RefCell, collections::HashMap, rc::Rc};

use rand::{rngs::StdRng, thread_rng, Rng, SeedableRng};
use triton_vm::{BFieldElement, NonDeterminism};

use crate::{
    linker::{execute_bench, link_for_isolated_run},
    snippet_bencher::{write_benchmarks, BenchmarkCase, BenchmarkResult},
    test_helpers::test_rust_equivalence_given_complete_state,
    VmHasherState,
};

use super::{basic_snippet::BasicSnippet, rust_shadow::RustShadow};

/// A Function is a piece of tasm code that can modify the top of the stack, and can read
/// and even extend memory. Specifically: any memory writes have to happen to addresses
/// larger than the dynamic memory allocator and the dynamic memory allocator value has to
/// be updated accordingly.
///
/// See also: [closure], [algorithm], [procedure]
///
/// [closure]: crate::traits::closure::Closure
/// [algorithm]: crate::traits::algorithm::Algorithm
/// [procedure]: crate::traits::procedure::Procedure
pub trait Function: BasicSnippet {
    fn rust_shadow(
        &self,
        stack: &mut Vec<BFieldElement>,
        memory: &mut HashMap<BFieldElement, BFieldElement>,
    );

    /// Return (init_stack, init_memory)
    fn pseudorandom_initial_state(
        &self,
        seed: [u8; 32],
        bench_case: Option<BenchmarkCase>,
    ) -> FunctionInitialState;

    fn corner_case_initial_states(&self) -> Vec<FunctionInitialState> {
        vec![]
    }
}

#[derive(Debug, Clone, Default)]
pub struct FunctionInitialState {
    pub stack: Vec<BFieldElement>,
    pub memory: HashMap<BFieldElement, BFieldElement>,
}

pub struct ShadowedFunction<F: Function + 'static> {
    pub function: Rc<RefCell<F>>,
}

impl<F: Function + 'static> ShadowedFunction<F> {
    pub fn new(function: F) -> Self {
        Self {
            function: Rc::new(RefCell::new(function)),
        }
    }
}

impl<P: Function + 'static> ShadowedFunction<P> {
    fn test_initial_state(&self, state: FunctionInitialState) {
        let FunctionInitialState { stack, memory } = state;

        let stdin = vec![];
        let non_determinism = NonDeterminism {
            individual_tokens: vec![],
            digests: vec![],
            ram: memory,
        };
        test_rust_equivalence_given_complete_state(
            self,
            &stack,
            &stdin,
            &non_determinism,
            &None,
            0,
            None,
        );
    }
}

impl<F> RustShadow for ShadowedFunction<F>
where
    F: Function + 'static,
{
    fn rust_shadow_wrapper(
        &self,
        _stdin: &[BFieldElement],
        _nondeterminism: &triton_vm::NonDeterminism<BFieldElement>,
        stack: &mut Vec<BFieldElement>,
        memory: &mut HashMap<BFieldElement, BFieldElement>,
        _sponge_state: &mut Option<VmHasherState>,
    ) -> Vec<BFieldElement> {
        self.function.borrow().rust_shadow(stack, memory);
        vec![]
    }

    /// Test rust-tasm equivalence.
    fn test(&self) {
        let entrypoint = self.function.borrow().entrypoint();
        for (i, cornercase_test) in self
            .function
            .borrow()
            .corner_case_initial_states()
            .into_iter()
            .enumerate()
        {
            println!("testing {entrypoint} corner case number {i}");
            self.test_initial_state(cornercase_test);
        }

        let num_rng_states = 5;
        let mut rng = thread_rng();

        for _ in 0..num_rng_states {
            let seed: [u8; 32] = rng.gen();
            println!("testing {entrypoint} with seed: {:x?}", seed);
            self.test_initial_state(
                self.function
                    .borrow()
                    .pseudorandom_initial_state(seed, None),
            )
        }
    }

    /// Count number of cycles and other performance indicators and save them in directory
    /// benchmarks/.
    fn bench(&self) {
        let mut rng: StdRng = SeedableRng::from_seed(
            hex::decode("73a24b6b8b32e4d7d563a4d9a85f476573a24b6b8b32e4d7d563a4d9a85f4765")
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let mut benchmarks = Vec::with_capacity(2);

        for bench_case in [BenchmarkCase::CommonCase, BenchmarkCase::WorstCase] {
            let FunctionInitialState { stack, memory } = self
                .function
                .borrow()
                .pseudorandom_initial_state(rng.gen(), Some(bench_case));
            let program = link_for_isolated_run(self.function.clone(), 1);
            let non_determinism = NonDeterminism::default().with_ram(memory);
            let execution_result = execute_bench(&program, &stack, vec![], non_determinism, None);
            let benchmark = BenchmarkResult {
                name: self.function.borrow().entrypoint(),
                clock_cycle_count: execution_result.cycle_count,
                hash_table_height: execution_result.hash_table_height,
                u32_table_height: execution_result.u32_table_height,
                case: bench_case,
            };
            benchmarks.push(benchmark);
        }

        write_benchmarks(benchmarks);
    }

    fn inner(&self) -> Rc<RefCell<dyn BasicSnippet>> {
        self.function.clone()
    }
}

use rand::{rngs::StdRng, SeedableRng};
use std::collections::HashMap;
use triton_vm::{instruction::LabelledInstruction, triton_asm, NonDeterminism};
use twenty_first::shared_math::b_field_element::BFieldElement;

use crate::data_type::DataType;
use crate::traits::basic_snippet::BasicSnippet;
use crate::traits::procedure::{Procedure, ProcedureInitialState};
use crate::{empty_stack, VmHasherState};

pub struct WriteToStdout {
    pub data_type: DataType,
}

impl BasicSnippet for WriteToStdout {
    fn inputs(&self) -> Vec<(DataType, String)> {
        vec![(self.data_type.clone(), "value".to_string())]
    }

    fn outputs(&self) -> Vec<(DataType, String)> {
        vec![]
    }

    fn entrypoint(&self) -> String {
        format!(
            "tasm_io_write_to_stdout___{}",
            self.data_type.label_friendly_name()
        )
    }

    fn code(&self, _library: &mut crate::library::Library) -> Vec<LabelledInstruction> {
        triton_asm!(
            {self.entrypoint()}:
                {&self.data_type.write_value_to_stdout()}
                return
        )
    }
}

impl Procedure for WriteToStdout {
    fn rust_shadow(
        &self,
        stack: &mut Vec<BFieldElement>,
        _memory: &mut HashMap<BFieldElement, BFieldElement>,
        _nondeterminism: &NonDeterminism<BFieldElement>,
        _public_input: &[BFieldElement],
        _sponge_state: &mut Option<VmHasherState>,
    ) -> Vec<BFieldElement> {
        let mut ret = vec![];
        for _ in 0..self.data_type.stack_size() {
            let value = stack.pop().unwrap();
            ret.push(value);
        }
        ret
    }

    fn pseudorandom_initial_state(
        &self,
        seed: [u8; 32],
        _bench_case: Option<crate::snippet_bencher::BenchmarkCase>,
    ) -> ProcedureInitialState {
        let mut rng: StdRng = SeedableRng::from_seed(seed);
        let mut stack = empty_stack();
        let random_value = self.data_type.seeded_random_elements(1, &mut rng);
        for elem in random_value[0].clone().into_iter().rev() {
            stack.push(elem);
        }

        ProcedureInitialState {
            stack,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::procedure::ShadowedProcedure;
    use crate::traits::rust_shadow::RustShadow;

    #[test]
    fn write_to_stdout_auto_test() {
        for data_type in DataType::big_random_generatable_type_collection() {
            ShadowedProcedure::new(WriteToStdout { data_type }).test();
        }
    }
}

#[cfg(test)]
mod benches {
    use super::*;
    use crate::traits::procedure::ShadowedProcedure;
    use crate::traits::rust_shadow::RustShadow;

    #[test]
    fn bench_for_digest_writing() {
        ShadowedProcedure::new(WriteToStdout {
            data_type: DataType::Digest,
        })
        .bench();
    }
}

use std::collections::HashMap;

use rand::{thread_rng, Rng, RngCore};
use twenty_first::shared_math::b_field_element::BFieldElement;

use crate::data_type::DataType;
use crate::library::Library;
use crate::traits::deprecated_snippet::DeprecatedSnippet;
use crate::{empty_stack, ExecutionState};

#[derive(Clone, Debug)]
pub struct Shiftleft;

impl DeprecatedSnippet for Shiftleft {
    fn entrypoint_name(&self) -> String {
        "tasm_arithmetic_u32_shiftleft".to_string()
    }

    fn input_field_names(&self) -> Vec<String> {
        vec!["value".to_string(), "shift".to_string()]
    }

    fn input_types(&self) -> Vec<DataType> {
        vec![DataType::U32, DataType::U32]
    }

    fn output_types(&self) -> Vec<DataType> {
        vec![DataType::U32]
    }

    fn output_field_names(&self) -> Vec<String> {
        vec!["value << shift".to_string()]
    }

    fn stack_diff(&self) -> isize {
        -1
    }

    fn function_code(&self, _library: &mut Library) -> String {
        let entrypoint = self.entrypoint_name();

        // I'm unsure if we should do a bounds check to check if `shift < 32`
        format!(
            "
            // BEFORE: _ value shift
            // AFTER:  _ (value << shift)
            {entrypoint}:
                // Bounds check. May be superfluous but this mimics Rust's behavior.
                push 32
                dup 1
                lt
                assert

                push 2
                pow
                mul
                split
                swap 1
                pop 1

                return
                "
        )
    }

    fn crash_conditions(&self) -> Vec<String> {
        vec![
            "inputs are not valid u32s".to_string(),
            "attempting to left shift with a value greater than 31".to_string(),
        ]
    }

    fn gen_input_states(&self) -> Vec<crate::ExecutionState> {
        let mut rng = thread_rng();
        let mut ret: Vec<ExecutionState> = vec![];
        for _ in 0..100 {
            let value = rng.next_u32();
            let shift = rng.gen_range(0..32);
            ret.push(prepare_state(value, shift));
        }

        ret
    }

    fn common_case_input_state(&self) -> ExecutionState {
        prepare_state((1 << 16) - 1, 16)
    }

    fn worst_case_input_state(&self) -> ExecutionState {
        prepare_state(u32::MAX, 31)
    }

    fn rust_shadowing(
        &self,
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        _memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        // Find shift amount
        let shift_amount: u32 = stack.pop().unwrap().try_into().unwrap();

        // Original value
        let value: u32 = stack.pop().unwrap().try_into().unwrap();

        let ret = value << shift_amount;
        stack.push((ret as u64).into());
    }
}

fn prepare_state(value: u32, shift: u32) -> ExecutionState {
    let mut stack = empty_stack();
    let value = BFieldElement::new(value as u64);
    let shift = BFieldElement::new(shift as u64);
    stack.push(value);
    stack.push(shift);

    ExecutionState::with_stack(stack)
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::{
        test_rust_equivalence_given_input_values_deprecated,
        test_rust_equivalence_multiple_deprecated,
    };

    use super::*;

    #[test]
    fn shift_left_test() {
        test_rust_equivalence_multiple_deprecated(&Shiftleft, true);
    }

    #[test]
    fn shift_left_max_value_test() {
        for i in 0..32 {
            prop_shift_left(u32::MAX, i);
        }
    }

    #[test]
    #[should_panic]
    fn shift_beyond_limit() {
        let mut init_stack = empty_stack();
        init_stack.push(BFieldElement::new(u32::MAX as u64));
        init_stack.push(32u64.into());
        Shiftleft
            .link_and_run_tasm_from_state_for_test(&mut ExecutionState::with_stack(init_stack));
    }

    fn prop_shift_left(value: u32, shift_amount: u32) {
        let mut init_stack = empty_stack();
        init_stack.push(BFieldElement::new(value as u64));
        init_stack.push(BFieldElement::new(shift_amount as u64));

        let expected_u32 = value << shift_amount;

        let mut expected_stack = empty_stack();
        expected_stack.push((expected_u32 as u64).into());

        test_rust_equivalence_given_input_values_deprecated(
            &Shiftleft,
            &init_stack,
            &[],
            HashMap::default(),
            0,
            Some(&expected_stack),
        );
    }
}

#[cfg(test)]
mod benches {
    use super::*;
    use crate::snippet_bencher::bench_and_write;

    #[test]
    fn shift_left_benchmark() {
        bench_and_write(Shiftleft);
    }
}

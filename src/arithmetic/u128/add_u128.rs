use std::collections::HashMap;

use num::Zero;
use rand::Rng;
use twenty_first::{
    amount::u32s::U32s, shared_math::b_field_element::BFieldElement,
    util_types::algebraic_hasher::Hashable,
};

use crate::{
    get_init_tvm_stack,
    library::Library,
    push_hashable,
    snippet::{DataType, Snippet},
    ExecutionState,
};

#[derive(Clone)]
pub struct AddU128;

impl Snippet for AddU128 {
    fn inputs(&self) -> Vec<String> {
        vec![
            "rhs_3".to_string(),
            "rhs_2".to_string(),
            "rhs_1".to_string(),
            "rhs_0".to_string(),
            "lhs_3".to_string(),
            "lhs_2".to_string(),
            "lhs_1".to_string(),
            "lhs_0".to_string(),
        ]
    }

    fn outputs(&self) -> Vec<String> {
        vec![
            "(lhs + rhs)_3".to_string(),
            "(lhs + rhs)_2".to_string(),
            "(lhs + rhs)_1".to_string(),
            "(lhs + rhs)_0".to_string(),
        ]
    }

    fn input_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::U128, DataType::U128]
    }

    fn output_types(&self) -> Vec<crate::snippet::DataType> {
        vec![DataType::U128]
    }

    fn crash_conditions() -> Vec<String> {
        vec!["if (lhs + rhs) overflows u128".to_string()]
    }

    fn gen_input_states(&self) -> Vec<ExecutionState> {
        let mut rng = rand::thread_rng();

        let mut states = vec![];
        let zero = U32s::<4>::zero();

        for _ in 0..20 {
            let small_a = U32s::<4>::try_from(rng.gen::<u64>()).unwrap();
            let small_b = U32s::<4>::try_from(rng.gen::<u64>()).unwrap();
            let mut random_bytes: [u32; 4] = [0, 0, 0, 0];
            rng.fill(&mut random_bytes);
            let large_a = U32s::<4>::new(random_bytes);

            // 0. one zero, one large
            states.push({
                let mut stack = get_init_tvm_stack();
                push_hashable(&mut stack, &zero);
                push_hashable(&mut stack, &large_a);
                ExecutionState::with_stack(stack)
            });

            // 1. two small
            states.push({
                let mut stack = get_init_tvm_stack();
                push_hashable(&mut stack, &small_a);
                push_hashable(&mut stack, &small_b);
                ExecutionState::with_stack(stack)
            });
        }

        states
    }

    fn stack_diff(&self) -> isize {
        -4
    }

    fn entrypoint(&self) -> String {
        "tasm_arithmetic_u128_add".to_string()
    }

    /// Four top elements of stack are assumed to be valid u32s. So to have
    /// a value that's less than 2^32.
    fn function_body(&self, _library: &mut Library) -> String {
        let entrypoint = self.entrypoint();
        format!(
            "
            // BEFORE: _ rhs_3 rhs_2 rhs_1 rhs_0 lhs_3 lhs_2 lhs_1 lhs_0
            // AFTER: _ sum_3 sum_2 sum_1 sum_0
            {entrypoint}:
                swap1 swap4
                add
                // _ rhs_3 rhs_2 rhs_1 lhs_1 lhs_3 lhs_2 (lhs_0 + rhs_0)

                split
                // _ rhs_3 rhs_2 rhs_1 lhs_1 lhs_3 lhs_2 (lhs_0 + rhs_0)_hi (lhs_0 + rhs_0)_lo

                // rename:
                // _ rhs_3 rhs_2 rhs_1 lhs_1 lhs_3 lhs_2 carry_1 sum_0

                swap4
                // _ rhs_3 rhs_2 rhs_1 sum_0 lhs_3 lhs_2 carry_1 lhs_1

                add
                // _ rhs_3 rhs_2 rhs_1 sum_0 lhs_3 lhs_2 lhs_1'

                swap1 swap4
                // _ rhs_3 rhs_2 lhs_2 sum_0 lhs_3 lhs_1' rhs_1

                add
                // _ rhs_3 rhs_2 lhs_2 sum_0 lhs_3 (lhs_1' + rhs_1)

                split
                // _ rhs_3 rhs_2 lhs_2 sum_0 lhs_3 carry_2 sum_1

                swap4
                // _ rhs_3 rhs_2 sum_1 sum_0 lhs_3 carry_2 lhs_2

                add
                // _ rhs_3 rhs_2 sum_1 sum_0 lhs_3 lhs_2'

                swap1 swap4
                // _ rhs_3 lhs_3 sum_1 sum_0 lhs_2' rhs_2

                add
                // _ rhs_3 lhs_3 sum_1 sum_0 (lhs_2' + rhs_2)

                split
                // _ rhs_3 lhs_3 sum_1 sum_0 carry_3 sum_2

                swap4
                // _ rhs_3 sum_2 sum_1 sum_0 carry_3 lhs_3

                add
                // _ rhs_3 sum_2 sum_1 sum_0 lhs_3'

                dup4
                // _ rhs_3 sum_2 sum_1 sum_0 lhs_3' rhs_3

                add
                // _ rhs_3 sum_2 sum_1 sum_0 (lhs_3' + rhs_3)

                split
                // _ rhs_3 sum_2 sum_1 sum_0 overflow sum_3

                swap5
                pop
                // _ sum_3 sum_2 sum_1 sum_0 overflow

                push 0
                eq
                assert
                // _ sum_3 sum_2 sum_1 sum_0

                return
            "
        )
    }

    fn rust_shadowing(
        &self,
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        _memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        // top element on stack
        let a0: u32 = stack.pop().unwrap().try_into().unwrap();
        let b0: u32 = stack.pop().unwrap().try_into().unwrap();
        let c0: u32 = stack.pop().unwrap().try_into().unwrap();
        let d0: u32 = stack.pop().unwrap().try_into().unwrap();
        let ab0 = U32s::<4>::new([a0, b0, c0, d0]);

        // second element on stack
        let a1: u32 = stack.pop().unwrap().try_into().unwrap();
        let b1: u32 = stack.pop().unwrap().try_into().unwrap();
        let c1: u32 = stack.pop().unwrap().try_into().unwrap();
        let d1: u32 = stack.pop().unwrap().try_into().unwrap();
        let ab1 = U32s::<4>::new([a1, b1, c1, d1]);
        let ab0_plus_ab1 = ab0 + ab1;
        let mut res = ab0_plus_ab1.to_sequence();
        for _ in 0..res.len() {
            stack.push(res.pop().unwrap());
        }
    }

    fn common_case_input_state(&self) -> ExecutionState
    where
        Self: Sized,
    {
        ExecutionState::with_stack(
            vec![
                get_init_tvm_stack(),
                vec![BFieldElement::zero(), BFieldElement::new(1 << 31)],
                vec![BFieldElement::zero(), BFieldElement::new(1 << 30)],
                vec![BFieldElement::zero(), BFieldElement::new(1 << 30)],
                vec![BFieldElement::zero(), BFieldElement::new(1 << 30)],
            ]
            .concat(),
        )
    }

    fn worst_case_input_state(&self) -> ExecutionState
    where
        Self: Sized,
    {
        ExecutionState::with_stack(
            vec![
                get_init_tvm_stack(),
                vec![BFieldElement::zero(), BFieldElement::new(1 << 31)],
                vec![BFieldElement::zero(), BFieldElement::new(1 << 30)],
                vec![BFieldElement::zero(), BFieldElement::new(1 << 30)],
                vec![BFieldElement::zero(), BFieldElement::new(1 << 30)],
            ]
            .concat(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::snippet_bencher::bench_and_write;
    use crate::test_helpers::{rust_tasm_equivalence_prop, rust_tasm_equivalence_prop_new};

    use super::*;

    #[test]
    fn add_u128_test() {
        rust_tasm_equivalence_prop_new(AddU128);
    }

    #[test]
    fn add_u128_benchmark() {
        bench_and_write(AddU128);
    }

    #[test]
    fn add_u128_unit_test() {
        let mut expected = get_init_tvm_stack();
        expected.push(BFieldElement::new(0));
        expected.push(BFieldElement::new(1 << 4));
        expected.push(BFieldElement::new(0));
        expected.push(BFieldElement::new(0));
        prop_add(1u128 << 67, 1u128 << 67, Some(&expected))
    }

    fn prop_add(lhs: u128, rhs: u128, expected: Option<&[BFieldElement]>) {
        let mut init_stack = get_init_tvm_stack();
        for elem in rhs.to_sequence().into_iter().rev() {
            init_stack.push(elem);
        }
        for elem in lhs.to_sequence().into_iter().rev() {
            init_stack.push(elem);
        }

        let _execution_result = rust_tasm_equivalence_prop::<AddU128>(
            AddU128,
            &init_stack,
            &[],
            &[],
            &mut HashMap::default(),
            0,
            expected,
        );
    }
}

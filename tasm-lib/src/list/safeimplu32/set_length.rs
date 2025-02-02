use std::collections::HashMap;

use itertools::Itertools;
use num::One;
use rand::{random, thread_rng, Rng};
use triton_vm::triton_asm;
use twenty_first::shared_math::b_field_element::BFieldElement;

use crate::data_type::DataType;
use crate::library::Library;
use crate::rust_shadowing_helper_functions::safe_list::safe_insert_random_list;
use crate::traits::deprecated_snippet::DeprecatedSnippet;
use crate::{empty_stack, ExecutionState};

#[derive(Clone, Debug)]
pub struct SafeSetLength {
    pub data_type: DataType,
}

impl DeprecatedSnippet for SafeSetLength {
    fn entrypoint_name(&self) -> String {
        format!(
            "tasm_list_safeimplu32_set_length___{}",
            self.data_type.label_friendly_name()
        )
    }

    fn input_field_names(&self) -> Vec<String> {
        vec!["*list".to_string(), "list_length".to_string()]
    }

    fn input_types(&self) -> Vec<DataType> {
        vec![
            DataType::List(Box::new(self.data_type.clone())),
            DataType::U32,
        ]
    }

    fn output_field_names(&self) -> Vec<String> {
        vec!["*list".to_string()]
    }

    fn output_types(&self) -> Vec<DataType> {
        vec![DataType::List(Box::new(self.data_type.clone()))]
    }

    fn stack_diff(&self) -> isize {
        // pops list_length but leaves list_pointer on stack
        -1
    }

    fn function_code(&self, _library: &mut Library) -> String {
        let entry_point = self.entrypoint_name();
        // It is assumed that the new length is a valid u32 value
        triton_asm!(
                // BEFORE: _ *list list_length
                // AFTER: _ *list
                {entry_point}:
                    dup 0
                    swap 2
                    // _ list_length list_length *list

                    write_mem 1
                    // _ list_length (*list + 1)

                    read_mem 1
                    // _ list_length capacity *list

                    swap 2
                    // _ *list capacity list_length

                    swap 1
                    // _ *list list_length capacity

                    lt
                    // _ *list (list_length > capacity)

                    push 0
                    eq
                    // _ *list (list_length <= capacity)

                    assert
                    // _ *list

                    return
        )
        .iter()
        .join("\n")
    }

    fn crash_conditions(&self) -> Vec<String> {
        vec!["New length exceeds capacity".to_string()]
    }

    fn gen_input_states(&self) -> Vec<ExecutionState> {
        let capacity = 100;
        vec![
            prepare_state(
                &self.data_type,
                capacity,
                thread_rng().gen_range(0..capacity) as usize,
                thread_rng().gen_range(0..capacity) as usize,
            ),
            prepare_state(
                &self.data_type,
                capacity,
                thread_rng().gen_range(0..capacity) as usize,
                thread_rng().gen_range(0..capacity) as usize,
            ),
            prepare_state(
                &self.data_type,
                capacity,
                thread_rng().gen_range(0..capacity) as usize,
                thread_rng().gen_range(0..capacity) as usize,
            ),
        ]
    }

    fn common_case_input_state(&self) -> ExecutionState {
        prepare_state(&self.data_type, 1000, 1 << 5, 1 << 4)
    }

    fn worst_case_input_state(&self) -> ExecutionState {
        prepare_state(&self.data_type, 1000, 1 << 6, 1 << 5)
    }

    fn rust_shadowing(
        &self,
        stack: &mut Vec<BFieldElement>,
        _std_in: Vec<BFieldElement>,
        _secret_in: Vec<BFieldElement>,
        memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        let new_length = stack.pop().unwrap();
        let new_length_u32 = new_length.value().try_into().unwrap();
        let list_address = stack.pop().unwrap();

        let capacity: u32 = memory[&(list_address + BFieldElement::one())]
            .value()
            .try_into()
            .unwrap();
        assert!(
            capacity >= new_length_u32,
            "New length cannot exceed list's capacity"
        );

        memory.insert(list_address, new_length);

        stack.push(list_address);
    }
}

fn prepare_state(
    data_type: &DataType,
    capacity: u32,
    init_length: usize,
    new_length: usize,
) -> ExecutionState {
    let list_pointer: BFieldElement = random();
    let mut stack = empty_stack();
    stack.push(list_pointer);
    stack.push(BFieldElement::new(new_length as u64));
    let mut memory = HashMap::default();
    safe_insert_random_list(data_type, list_pointer, capacity, init_length, &mut memory);
    ExecutionState::with_stack_and_memory(stack, memory, 0)
}

#[cfg(test)]
mod tests {
    use twenty_first::shared_math::b_field_element::BFieldElement;

    use crate::empty_stack;
    use crate::test_helpers::{
        test_rust_equivalence_given_input_values_deprecated,
        test_rust_equivalence_multiple_deprecated,
    };

    use super::*;

    #[test]
    fn new_snippet_test() {
        fn test_rust_equivalence_and_export(data_type: DataType) {
            test_rust_equivalence_multiple_deprecated(&SafeSetLength { data_type }, true);
        }

        test_rust_equivalence_and_export(DataType::Bool);
        test_rust_equivalence_and_export(DataType::U32);
        test_rust_equivalence_and_export(DataType::U64);
        test_rust_equivalence_and_export(DataType::Bfe);
        test_rust_equivalence_and_export(DataType::Xfe);
        test_rust_equivalence_and_export(DataType::Digest);
    }

    #[test]
    fn list_u32_n_is_one_decrease() {
        let list_address = BFieldElement::new(58);
        let init_length = 22;
        let new_list_length = 14;
        let capacity = 22;
        prop_set_length(
            DataType::Bfe,
            list_address,
            init_length,
            new_list_length,
            capacity,
        );
    }

    #[test]
    fn list_u32_n_is_one_increase() {
        let list_address = BFieldElement::new(58);
        let init_length = 2;
        let new_list_length = 22;
        let capacity = 22;
        prop_set_length(
            DataType::Bfe,
            list_address,
            init_length,
            new_list_length,
            capacity,
        );
    }

    #[should_panic]
    #[test]
    fn list_u32_n_is_one_increase_beyond_capacity_a() {
        let list_pointer = BFieldElement::new(1841);
        let init_length = 20;
        let new_list_length = 21;
        let capacity = 20;
        prop_set_length(
            DataType::U32,
            list_pointer,
            init_length,
            new_list_length,
            capacity,
        );
    }

    #[should_panic]
    #[test]
    fn list_u32_n_is_one_increase_beyond_capacity_b() {
        let list_pointer = BFieldElement::new(1841);
        let init_length = 20;
        let new_list_length = 22;
        let capacity = 20;
        prop_set_length(
            DataType::U32,
            list_pointer,
            init_length,
            new_list_length,
            capacity,
        );
    }

    #[should_panic]
    #[test]
    fn list_u32_n_is_one_increase_beyond_capacity_c() {
        let list_pointer = BFieldElement::new(1841);
        let init_length = 20;
        let new_list_length = 21;
        let capacity = 20;
        prop_set_length(
            DataType::Xfe,
            list_pointer,
            init_length,
            new_list_length,
            capacity,
        );
    }

    #[test]
    fn list_u32_n_is_five_push() {
        let list_address = BFieldElement::new(558);
        let init_length = 231;
        let new_list_length = 14;
        let capacity = 300;
        prop_set_length(
            DataType::Digest,
            list_address,
            init_length,
            new_list_length,
            capacity,
        );

        let init_length = 14;
        let new_list_length = 0;
        let capacity = 300;
        prop_set_length(
            DataType::Digest,
            list_address,
            init_length,
            new_list_length,
            capacity,
        );

        let init_length = 0;
        let new_list_length = 0;
        let capacity = 300;
        prop_set_length(
            DataType::Digest,
            list_address,
            init_length,
            new_list_length,
            capacity,
        );
    }

    fn prop_set_length(
        data_type: DataType,
        list_pointer: BFieldElement,
        init_list_length: u32,
        new_list_length: u32,
        capacity: u32,
    ) {
        let expected_end_stack = [empty_stack(), vec![list_pointer]].concat();
        let mut init_stack = empty_stack();
        init_stack.push(list_pointer);
        init_stack.push(BFieldElement::new(new_list_length as u64));

        let mut memory = HashMap::default();

        // Insert length indicator of list, lives on offset = 0 from `list_address`
        safe_insert_random_list(
            &data_type,
            list_pointer,
            capacity,
            init_list_length as usize,
            &mut memory,
        );

        let memory = test_rust_equivalence_given_input_values_deprecated::<SafeSetLength>(
            &SafeSetLength { data_type },
            &init_stack,
            &[],
            memory,
            0,
            Some(&expected_end_stack),
        )
        .final_ram;

        // Verify that length indicator has been updated
        assert_eq!(
            BFieldElement::new(new_list_length as u64),
            memory[&list_pointer]
        );
    }
}

#[cfg(test)]
mod benches {
    use crate::snippet_bencher::bench_and_write;

    use super::*;

    #[test]
    fn safe_set_length_benchmark() {
        bench_and_write(SafeSetLength {
            data_type: DataType::Digest,
        });
    }
}

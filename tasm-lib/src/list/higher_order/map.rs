use itertools::Itertools;
use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng};
use std::collections::HashMap;
use triton_vm::parser::tokenize;
use triton_vm::{triton_asm, NonDeterminism};
use twenty_first::shared_math::b_field_element::BFieldElement;
use twenty_first::shared_math::other::random_elements;

use crate::function::Function;
use crate::list::safe_u32::get::SafeGet;
use crate::list::safe_u32::length::SafeLength;
use crate::list::safe_u32::new::SafeNew;
use crate::list::safe_u32::set::SafeSet;
use crate::list::safe_u32::set_length::SafeSetLength;
use crate::list::unsafe_u32::get::UnsafeGet;
use crate::list::unsafe_u32::length::UnsafeLength;
use crate::list::unsafe_u32::new::UnsafeNew;
use crate::list::unsafe_u32::set::UnsafeSet;
use crate::list::unsafe_u32::set_length::UnsafeSetLength;
use crate::list::{self, ListType};
use crate::rust_shadowing_helper_functions::safe_list::safe_insert_random_list;
use crate::rust_shadowing_helper_functions::unsafe_list::unsafe_insert_random_list;
use crate::snippet::BasicSnippet;
use crate::{get_init_tvm_stack, rust_shadowing_helper_functions};
use crate::{
    library::Library,
    snippet::{DataType, DeprecatedSnippet},
    ExecutionState,
};

use super::inner_function::InnerFunction;

/// Applies a given function to every element of a list, and collects the new elements
/// into a new list.
pub struct Map {
    pub list_type: ListType,
    pub f: InnerFunction,
}

impl BasicSnippet for Map {
    fn inputs(&self) -> Vec<(DataType, String)> {
        match &self.f {
            InnerFunction::BasicSnippet(bs) => {
                let inner_type = &bs.inputs()[0].0;
                vec![(
                    DataType::List(Box::new(inner_type.clone())),
                    "*input_list".to_string(),
                )]
            }
            _ => vec![(
                DataType::List(Box::new(DataType::VoidPointer)),
                "*input_list".to_string(),
            )],
        }
    }

    fn outputs(&self) -> Vec<(DataType, String)> {
        match &self.f {
            InnerFunction::BasicSnippet(bs) => {
                let inner_type = &bs.outputs()[0].0;
                vec![(
                    DataType::List(Box::new(inner_type.clone())),
                    "*input_list".to_string(),
                )]
            }
            _ => vec![(
                DataType::List(Box::new(DataType::VoidPointer)),
                "*input_list".to_string(),
            )],
        }
    }

    fn entrypoint(&self) -> String {
        format!(
            "tasm_list_higher_order_{}_u32_map_{}",
            self.list_type,
            self.f.entrypoint()
        )
    }

    fn code(&self, library: &mut Library) -> Vec<triton_vm::instruction::LabelledInstruction> {
        let input_list_element_type = self.f.domain();
        let output_type = self.f.range();
        let output_size_plus_one = 1 + output_type.get_size();

        let get_length = match self.list_type {
            ListType::Safe => library.import(Box::new(SafeLength(input_list_element_type.clone()))),
            ListType::Unsafe => {
                library.import(Box::new(UnsafeLength(input_list_element_type.clone())))
            }
        };
        let set_length = match self.list_type {
            ListType::Safe => {
                library.import(Box::new(SafeSetLength(input_list_element_type.clone())))
            }
            ListType::Unsafe => {
                library.import(Box::new(UnsafeSetLength(input_list_element_type.clone())))
            }
        };
        let new_list = match self.list_type {
            ListType::Safe => library.import(Box::new(SafeNew(output_type.clone()))),
            ListType::Unsafe => library.import(Box::new(UnsafeNew(output_type.clone()))),
        };
        let list_get = match self.list_type {
            ListType::Safe => library.import(Box::new(SafeGet(input_list_element_type))),
            ListType::Unsafe => library.import(Box::new(UnsafeGet(input_list_element_type))),
        };
        let list_set = match self.list_type {
            ListType::Safe => library.import(Box::new(SafeSet(output_type))),
            ListType::Unsafe => library.import(Box::new(UnsafeSet(output_type))),
        };

        // Declare the inner function entrypoint name and import inner function in case it's a snippet
        let inner_function_name = match &self.f {
            InnerFunction::RawCode(rc) => rc.entrypoint(),
            InnerFunction::DeprecatedSnippet(sn) => {
                let fn_body = sn.function_code(library);
                let (_, instructions) = tokenize(&fn_body).unwrap();
                let labelled_instructions =
                    triton_vm::parser::to_labelled_instructions(&instructions);
                library.explicit_import(&sn.entrypoint_name(), &labelled_instructions)
            }
            InnerFunction::NoFunctionBody(lnat) => lnat.label_name.to_owned(),
            InnerFunction::BasicSnippet(bs) => {
                let labelled_instructions = bs.code(library);
                library.explicit_import(&bs.entrypoint(), &labelled_instructions)
            }
        };

        // If function was supplied as raw instructions, we need to append the inner function to the function
        // body. Otherwise, `library` handles the imports.
        let maybe_inner_function_body_raw = match &self.f {
            InnerFunction::RawCode(rc) => rc.function.iter().map(|x| x.to_string()).join("\n"),
            InnerFunction::DeprecatedSnippet(_) => String::default(),
            InnerFunction::NoFunctionBody(_) => String::default(),
            InnerFunction::BasicSnippet(_) => String::default(),
        };
        let entrypoint = self.entrypoint();
        let main_loop = format!("{entrypoint}_loop");

        triton_asm!(
            // BEFORE: _ <[additional_input_args]>  input_list
            // AFTER: _ <[additional_input_args]>  output_list
            {entrypoint}:

                dup 0                   // _ <aia>  input_list input_list
                call {get_length}       // _ <aia>  input_list len
                dup 0                   // _ <aia>  input_list len len
                call {new_list}         // _ <aia>  input_list len output_list
                dup 1                   // _ <aia>  input_list len output_list len
                call {set_length}       // _ <aia>  input_list len output_list
                swap 1                  // _ <aia>  input_list output_list len

                call {main_loop}        // _ <aia>  input_list output_list 0

                pop                     // _ <aia>  input_list output_list
                swap 1                  // _ <aia>  output_list input_list
                pop                     // _ <aia>  output_list

                return

            // INVARIANT: _ <aia>  input_list output_list itr
            {main_loop}:
                // test return condition
                dup 0                   // _ <aia>  input_list output_list itr itr
                push 0 eq               // _ <aia>  input_list output_list itr itr==0

                skiz return
                // _ input_list output_list itr

                // body
                push -1 add             // _ <aia>  input_list output_list index

                // read
                dup 2 dup 1             // _ <aia>  input_list output_list index _input_list index
                call {list_get}         // _ <aia>  input_list output_list index [input_element]

                // map
                call {inner_function_name} // _ <aia>  input_list output_list index [output_element]


                // write
                dup {output_size_plus_one} // _ <aia>  input_list output_list index [output_element] output_list
                dup {output_size_plus_one} // _ <aia>  input_list output_list index [output_element] output_list index

                call {list_set}            // _ <aia>  input_list output_list index

                recurse

            {maybe_inner_function_body_raw}
        )
    }
}

impl Function for Map {
    fn rust_shadow(
        &self,
        stack: &mut Vec<BFieldElement>,
        memory: &mut HashMap<BFieldElement, BFieldElement>,
    ) {
        let input_list_element_type = self.f.domain();
        let output_type = self.f.range();

        let list_pointer = stack.pop().unwrap();

        // get list length
        let len = match self.list_type {
            ListType::Safe => rust_shadowing_helper_functions::safe_list::safe_list_get_length(
                list_pointer,
                memory,
            ),
            ListType::Unsafe => {
                rust_shadowing_helper_functions::unsafe_list::unsafe_list_get_length(
                    list_pointer,
                    memory,
                )
            }
        };

        let list_element = match self.list_type {
            ListType::Safe => rust_shadowing_helper_functions::safe_list::safe_list_get,
            ListType::Unsafe => rust_shadowing_helper_functions::unsafe_list::unsafe_list_get,
        };

        let set_element = match self.list_type {
            ListType::Safe => rust_shadowing_helper_functions::safe_list::safe_list_set,
            ListType::Unsafe => rust_shadowing_helper_functions::unsafe_list::unsafe_list_set,
        };

        // used for deprecated rust shadowers
        let std_in = vec![];
        let secret_in = vec![];

        let output_list_capacity = len;
        let output_list =
            match self.list_type {
                ListType::Safe => {
                    // Push capacity to stack
                    stack.push(BFieldElement::new(output_list_capacity as u64));
                    list::safe_u32::new::SafeNew(input_list_element_type.clone()).rust_shadowing(
                        stack,
                        std_in.clone(),
                        secret_in.clone(),
                        memory,
                    );
                    stack.pop().unwrap()
                }
                ListType::Unsafe => {
                    stack.push(BFieldElement::new(output_list_capacity as u64));
                    list::unsafe_u32::new::UnsafeNew(input_list_element_type.clone())
                        .rust_shadowing(stack, std_in.clone(), secret_in.clone(), memory);
                    stack.pop().unwrap()
                }
            };

        // set length
        stack.push(output_list);
        stack.push(BFieldElement::new(len as u64));
        match self.list_type {
            ListType::Safe => {
                list::safe_u32::set_length::SafeSetLength(output_type.clone())
                    .rust_shadowing(stack, std_in, secret_in, memory);
            }
            ListType::Unsafe => {
                list::unsafe_u32::set_length::UnsafeSetLength(output_type.clone())
                    .rust_shadowing(stack, std_in, secret_in, memory);
            }
        }
        stack.pop();

        // Push three values that may not be changed by the inner function
        let canary_count = 3;
        let canaries: Vec<BFieldElement> = random_elements(canary_count);
        stack.append(&mut canaries.clone());

        // forall elements, read + map + write
        for i in 0..len {
            // read
            let mut input_item =
                list_element(list_pointer, i, memory, input_list_element_type.get_size());

            // put on stack
            while let Some(element) = input_item.pop() {
                stack.push(element);
            }

            self.f.apply(stack, memory);

            // pull from stack
            let mut output_item = vec![];
            for _ in 0..output_type.get_size() {
                output_item.push(stack.pop().unwrap());
            }

            // write
            set_element(output_list, i, output_item, memory, output_type.get_size());
        }

        // Ensure canaries are still on the stack, then remove them
        for i in 0..canary_count {
            assert_eq!(canaries[canary_count - i - 1], stack.pop().unwrap());
        }

        stack.push(output_list);
    }

    fn pseudorandom_initial_state(
        &self,
        seed: [u8; 32],
        _bench_case: Option<crate::snippet_bencher::BenchmarkCase>,
    ) -> (Vec<BFieldElement>, HashMap<BFieldElement, BFieldElement>) {
        let mut rng: StdRng = SeedableRng::from_seed(seed);
        let list_pointer = BFieldElement::new(rng.next_u64() % (1 << 25));
        let list_length = (rng.next_u32() % (1 << 6)) as usize;

        // Autogenerating these extra arguments seems pretty shady to me. Are they
        // u32s, BFEs, or XFEs? That depends on the inner function!
        let num_additional_function_args = (rng.next_u32() % 7) as usize;
        let additional_function_args = (0..num_additional_function_args)
            .map(|_| BFieldElement::new((rng.gen::<u32>() / 2) as u64))
            .collect_vec();
        let execution_state =
            self.generate_input_state(list_pointer, list_length, additional_function_args);
        (execution_state.stack, execution_state.memory)
    }
}

impl Map {
    fn generate_input_state(
        &self,
        list_pointer: BFieldElement,
        list_length: usize,
        additional_function_args: Vec<BFieldElement>,
    ) -> ExecutionState {
        let capacity = list_length;
        let mut stack = get_init_tvm_stack();

        // Add additional input args to stack, if they exist
        for additional_function_arg in additional_function_args.into_iter().rev() {
            stack.push(additional_function_arg);
        }

        stack.push(list_pointer);

        let mut memory = HashMap::default();
        let input_element_type = self.f.domain();
        let input_list_size = match self.list_type {
            ListType::Safe => {
                BFieldElement::new((2 + list_length * input_element_type.get_size()) as u64)
            }
            ListType::Unsafe => {
                BFieldElement::new((1 + list_length * input_element_type.get_size()) as u64)
            }
        };
        rust_shadowing_helper_functions::dyn_malloc::rust_dyn_malloc_initialize(
            &mut memory,
            (input_list_size + list_pointer).value() as usize,
        );

        match self.list_type {
            ListType::Safe => safe_insert_random_list(
                &input_element_type,
                list_pointer,
                capacity as u32,
                list_length,
                &mut memory,
            ),
            ListType::Unsafe => unsafe_insert_random_list(
                &input_element_type,
                list_pointer,
                list_length,
                &mut memory,
            ),
        };

        ExecutionState {
            stack,
            std_in: vec![],
            nondeterminism: NonDeterminism::new(vec![]),
            memory,
            words_allocated: 0,
        }
    }
}

#[cfg(test)]
mod tests {

    use num_traits::Zero;
    use triton_vm::triton_asm;
    use twenty_first::{
        shared_math::other::random_elements, util_types::algebraic_hasher::AlgebraicHasher,
    };

    use crate::{
        arithmetic, function::ShadowedFunction, list::higher_order::inner_function::RawCode,
        snippet::RustShadow, VmHasher,
    };

    use super::*;

    #[derive(Debug, Clone)]
    pub(crate) struct TestHashXFieldElement;

    impl DeprecatedSnippet for TestHashXFieldElement {
        fn entrypoint_name(&self) -> String {
            "test_hash_xfield_element".to_string()
        }

        fn input_field_names(&self) -> Vec<String>
        where
            Self: Sized,
        {
            vec![
                "elem2".to_string(),
                "elem1".to_string(),
                "elem0".to_string(),
            ]
        }

        fn input_types(&self) -> Vec<DataType> {
            vec![DataType::XFE]
        }

        fn output_types(&self) -> Vec<DataType> {
            vec![DataType::Digest]
        }

        fn output_field_names(&self) -> Vec<String>
        where
            Self: Sized,
        {
            vec![
                "digelem4".to_string(),
                "digelem3".to_string(),
                "digelem2".to_string(),
                "digelem1".to_string(),
                "digelem0".to_string(),
            ]
        }

        fn stack_diff(&self) -> isize
        where
            Self: Sized,
        {
            2
        }

        fn function_code(&self, library: &mut Library) -> String {
            let entrypoint = self.entrypoint_name();
            let unused_import = library.import(Box::new(arithmetic::u32::safe_add::SafeAdd));
            format!(
                "
        // BEFORE: _ x2 x1 x0
        // AFTER: _ d4 d3 d2 d1 d0
        {entrypoint}:
            push 0
            push 0
            push 0
            push 1 // _ x2 x1 x0 0 0 0 1
            push 0 swap 7 // _ 0 x1 x0 0 0 0 1 x2
            push 0 swap 7 // _ 0 0 x0 0 0 0 1 x2 x1
            push 0 swap 7 // _ 0 0 0 0 0 0 1 x2 x1 x0

            // Useless additions, to ensure that imports are accepted inside the map generated code
            push 0
            push 0
            call {unused_import}
            pop

            absorb_init
            squeeze // _ d9 d8 d7 d6 d5 d4 d3 d2 d1 d0
            swap 5 pop // _ d9 d8 d7 d6 d0 d4 d3 d2 d1
            swap 5 pop // _ d9 d8 d7 d1 d0 d4 d3 d2
            swap 5 pop
            swap 5 pop
            swap 5 pop
            return
        "
            )
        }

        fn crash_conditions(&self) -> Vec<String>
        where
            Self: Sized,
        {
            vec![]
        }

        fn gen_input_states(&self) -> Vec<ExecutionState>
        where
            Self: Sized,
        {
            vec![ExecutionState::with_stack(
                vec![
                    vec![BFieldElement::zero(); 16],
                    random_elements::<BFieldElement>(3),
                ]
                .concat(),
            )]
        }

        fn common_case_input_state(&self) -> ExecutionState
        where
            Self: Sized,
        {
            ExecutionState::with_stack(
                vec![
                    vec![BFieldElement::zero(); 16],
                    random_elements::<BFieldElement>(3),
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
                    vec![BFieldElement::zero(); 16],
                    random_elements::<BFieldElement>(3),
                ]
                .concat(),
            )
        }

        fn rust_shadowing(
            &self,
            stack: &mut Vec<BFieldElement>,
            _std_in: Vec<BFieldElement>,
            _secret_in: Vec<BFieldElement>,
            _memory: &mut HashMap<BFieldElement, BFieldElement>,
        ) where
            Self: Sized,
        {
            let mut xfield_element = vec![];
            for _ in 0..3 {
                xfield_element.push(stack.pop().unwrap());
            }
            let mut digest = VmHasher::hash_varlen(&xfield_element).values().to_vec();
            while let Some(element) = digest.pop() {
                stack.push(element);
            }
        }
    }

    #[test]
    fn unsafe_list_prop_test() {
        let snippet = Map {
            list_type: ListType::Unsafe,
            f: InnerFunction::DeprecatedSnippet(Box::new(TestHashXFieldElement)),
        };
        ShadowedFunction::new(snippet).test();
    }

    #[test]
    fn with_safe_list_prop_test() {
        let snippet = Map {
            list_type: ListType::Safe,
            f: InnerFunction::DeprecatedSnippet(Box::new(TestHashXFieldElement)),
        };
        ShadowedFunction::new(snippet).test();
    }

    #[test]
    fn test_with_raw_function_identity_on_bfe() {
        let rawcode = RawCode::new(
            triton_asm!(identity_bfe: return),
            DataType::BFE,
            DataType::BFE,
        );
        let snippet = Map {
            list_type: ListType::Unsafe,
            f: InnerFunction::RawCode(rawcode),
        };
        ShadowedFunction::new(snippet).test();
    }

    #[test]
    fn test_with_raw_function_square_on_bfe() {
        let rawcode = RawCode::new(
            triton_asm!(square_bfe: dup 0 mul return),
            DataType::BFE,
            DataType::BFE,
        );
        let snippet = Map {
            list_type: ListType::Unsafe,
            f: InnerFunction::RawCode(rawcode),
        };
        ShadowedFunction::new(snippet).test();
    }

    #[test]
    fn test_with_raw_function_square_plus_n_on_bfe() {
        // Inner function calculates `|(n, x)| -> x*x + n`, where `x` is the list
        // element, and `n` is the same value for all elements.
        let rawcode = RawCode::new(
            triton_asm!(square_plus_n_bfe: dup 0 mul dup 4 add return),
            DataType::BFE,
            DataType::BFE,
        );
        let snippet = Map {
            list_type: ListType::Unsafe,
            f: InnerFunction::RawCode(rawcode),
        };
        ShadowedFunction::new(snippet).test();
    }

    #[test]
    fn test_with_raw_function_square_on_xfe() {
        let rawcode = RawCode::new(
            triton_asm!(
                square_xfe: dup 2 dup 2 dup 2 xxmul swap 3 pop swap 3 pop swap 3 pop return
            ),
            DataType::XFE,
            DataType::XFE,
        );
        let snippet = Map {
            list_type: ListType::Unsafe,
            f: InnerFunction::RawCode(rawcode),
        };
        ShadowedFunction::new(snippet).test();
    }

    #[test]
    fn test_with_raw_function_square_on_xfe_plus_another_xfe() {
        let rawcode = RawCode::new(
            triton_asm!(
                square_xfe_plus_another_xfe:
                    dup 2 dup 2 dup 2 xxmul
                    swap 3 pop swap 3 pop swap 3 pop
                    dup 8 dup 8 dup 8 xxadd
                    swap 3 pop swap 3 pop swap 3 pop
                    return
            ),
            DataType::XFE,
            DataType::XFE,
        );
        let snippet = Map {
            list_type: ListType::Unsafe,
            f: InnerFunction::RawCode(rawcode),
        };
        ShadowedFunction::new(snippet).test()
    }

    #[test]
    fn test_u32_list_to_u128_list_plus_x() {
        let rawcode = RawCode::new(
            triton_asm!(
                u32_to_u128_add_another_u128:
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index input_u32
                dup 4
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index input_u32 x_0
                add
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index (input_u32 + x_0)
                split
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index carry_to_1 output_0
                swap 1
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 carry_to_1
                dup 6
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 carry_to_1 x_1
                add
                split
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 carry_to_2 output_1
                swap 1
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 output_1 carry_to_2
                dup 8
                add
                split
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 output_1 carry_to_3 output_2
                swap 1
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 output_1 output_2 carry_to_3
                dup 10
                add
                split
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 output_1 output_2 overflow output_3
                swap 1
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 output_1 output_2 output_3 overflow

                // verify no overflow
                push 0
                eq
                assert
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_0 output_1 output_2 output_3
                swap 3
                swap 1
                swap 2
                swap 1
                // stack:  _ [x_3, x_2, x_1, x_0] input_list output_list index output_3 output_2 output_1 output_0
                return
            ),
            DataType::U32,
            DataType::U128,
        );
        ShadowedFunction::new(Map {
            list_type: ListType::Unsafe,
            f: InnerFunction::RawCode(rawcode),
        })
        .test();
    }
}

#[cfg(test)]
mod benches {
    use super::{tests::TestHashXFieldElement, *};
    use crate::{function::ShadowedFunction, snippet::RustShadow};

    #[test]
    fn unsafe_list_map_benchmark() {
        ShadowedFunction::new(Map {
            list_type: ListType::Unsafe,
            f: InnerFunction::DeprecatedSnippet(Box::new(TestHashXFieldElement)),
        })
        .bench();
    }

    #[test]
    fn safe_list_map_benchmark() {
        ShadowedFunction::new(Map {
            list_type: ListType::Safe,
            f: InnerFunction::DeprecatedSnippet(Box::new(TestHashXFieldElement)),
        })
        .bench();
    }
}

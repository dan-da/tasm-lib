use twenty_first::shared_math::b_field_element::BFieldElement;

use crate::snippet_trait::Snippet;

pub struct BfeAdd();

impl Snippet for BfeAdd {
    const STACK_DIFF: isize = -1;

    fn get_name() -> String {
        "bfe_add".to_string()
    }

    fn get_code() -> String {
        "add".to_string()
    }

    fn rust_shadowing(stack: &mut Vec<BFieldElement>) {
        let a = stack.pop().unwrap();
        let b = stack.pop().unwrap();
        stack.push(a + b);
    }
}

#[cfg(test)]
mod tests {
    use crate::get_init_tvm_stack;

    use super::*;

    #[test]
    fn bfe_add_simple() {
        // TASM
        let init_stack = vec![
            get_init_tvm_stack(),
            vec![BFieldElement::new(200), BFieldElement::new(300)],
        ]
        .concat();

        let mut tvm_stack = init_stack.clone();
        let execution_result = BfeAdd::run_tasm(&mut tvm_stack, vec![], vec![]);
        println!("Cycle count for `add`: {}", execution_result.cycle_count);

        // Rust
        let mut rust_stack = init_stack;
        BfeAdd::rust_shadowing(&mut rust_stack);

        // Check that the two functions agree
        assert_eq!(tvm_stack, rust_stack, "Rust code must match TVM for `add`");

        // Check that the expected result is returned
        assert_eq!(
            vec![get_init_tvm_stack(), vec![BFieldElement::new(500)]].concat(),
            tvm_stack,
            "Stack must match expected value"
        );
    }
}

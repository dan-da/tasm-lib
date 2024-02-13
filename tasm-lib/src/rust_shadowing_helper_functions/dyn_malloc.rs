use std::collections::HashMap;

use triton_vm::prelude::*;

use crate::empty_stack;
use crate::memory::dyn_malloc::DynMalloc;
use crate::traits::function::Function;

pub fn dynamic_allocator(memory: &mut HashMap<BFieldElement, BFieldElement>) -> BFieldElement {
    let mut init_stack = empty_stack();
    DynMalloc.rust_shadow(&mut init_stack, memory);
    init_stack.pop().unwrap()
}

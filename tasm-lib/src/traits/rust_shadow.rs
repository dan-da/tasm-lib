use std::{cell::RefCell, collections::HashMap, rc::Rc};

use triton_vm::{BFieldElement, NonDeterminism};

use crate::VmHasherState;

use super::basic_snippet::BasicSnippet;

pub trait RustShadow {
    fn inner(&self) -> Rc<RefCell<dyn BasicSnippet>>;

    fn rust_shadow_wrapper(
        &self,
        stdin: &[BFieldElement],
        nondeterminism: &NonDeterminism<BFieldElement>,
        stack: &mut Vec<BFieldElement>,
        memory: &mut HashMap<BFieldElement, BFieldElement>,
        sponge_state: &mut Option<VmHasherState>,
    ) -> Vec<BFieldElement>;

    fn test(&self);

    fn bench(&self);
}

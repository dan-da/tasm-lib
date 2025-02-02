use std::collections::HashMap;
use std::error::Error;

use itertools::Itertools;
use num_traits::Zero;
use triton_vm::{instruction::LabelledInstruction, BFieldElement};
use twenty_first::shared_math::bfield_codec::BFieldCodec;

pub use derive_tasm_object::TasmObject;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// TasmObject
///
/// This trait defines methods for dealing with custom-defined objects from within the VM,
/// assuming those methods live in memory as they are encoded with BFieldCodec.
///
/// The arguments referring to fields are strings. For structs with unnamed fields, the
/// nth field name is implicitly `field_n`.
pub trait TasmObject {
    /// Returns tasm code that returns a pointer the field of the object, assuming:
    ///  - that a pointer to the said object lives on top of the stack;
    ///  - said object has a type that implements the TasmObject trait;
    ///  - said object lives in memory encoded as BFieldCodec specifies.
    ///
    /// BEFORE: _ *object
    ///
    /// AFTER: _ *field
    fn get_field(field_name: &str) -> Vec<LabelledInstruction>;

    /// Returns tasm code that returns a pointer the field of the object, along with
    /// the size of that field in number of BFieldElements, assuming:
    ///  - that a pointer to the said object lives on top of the stack;
    ///  - said object has a type that implements the TasmObject trait;
    ///  - said object lives in memory encoded as BFieldCodec specifies.
    ///
    /// BEFORE: _ *object
    ///
    /// AFTER: _ *field field_size
    ///
    /// See also: `get_field` if you just want the field without the size.
    fn get_field_with_size(field_name: &str) -> Vec<LabelledInstruction>;

    /// Returns tasm code that returns a pointer to the start of the field of the object,
    /// along with the jump distance to the next field. Note that:
    ///
    ///  -  *field_start == *field      if the size is statically known, but
    ///  -  *field_start == *field-1    if the size is not statically known.
    ///
    /// BEFORE: _ *object
    ///
    /// AFTER: _ *field_start field_jump_distance
    ///
    /// This function is used internally for the derive macro. You probably want to use
    /// `get_field` or `get_field_with_size` instead.
    fn get_field_start_with_jump_distance(field_name: &str) -> Vec<LabelledInstruction>;

    /// Given an iterator over `BFieldElement`s, decode it as a Self object.
    fn decode_iter<Itr: Iterator<Item = BFieldElement>>(iterator: &mut Itr) -> Result<Box<Self>>;

    /// Given a memory object (as HashMap of BFE->BFE) and and address (BFE), decode the
    /// object located there.
    fn decode_from_memory(
        memory: &HashMap<BFieldElement, BFieldElement>,
        address: BFieldElement,
    ) -> Result<Box<Self>> {
        let mut iterator = MemoryIter::new(memory, address);
        Self::decode_iter(&mut iterator)
    }
}

pub fn decode_from_memory_with_size<T: BFieldCodec>(
    memory: &HashMap<BFieldElement, BFieldElement>,
    address: BFieldElement,
    size: usize,
) -> Result<Box<T>> {
    let sequence = (0..size)
        .map(|i| address + BFieldElement::new(i as u64))
        .map(|b| memory.get(&b).copied().unwrap_or(BFieldElement::new(0)))
        .collect_vec();
    T::decode(&sequence).map_err(|e| e.into())
}

impl<T: BFieldCodec> TasmObject for Vec<T> {
    fn get_field(_field_name: &str) -> Vec<LabelledInstruction> {
        panic!("`Vec` does not have fields; cannot access them")
    }

    fn get_field_with_size(_field_name: &str) -> Vec<LabelledInstruction> {
        panic!("`Vec` does not have fields; cannot access them")
    }

    fn get_field_start_with_jump_distance(_field_name: &str) -> Vec<LabelledInstruction> {
        panic!("`Vec` does not have fields; cannot access them")
    }

    fn decode_iter<Itr: Iterator<Item = BFieldElement>>(iterator: &mut Itr) -> Result<Box<Self>> {
        let length = iterator.next().unwrap().value() as usize;
        let mut vector = vec![];
        for _ in 0..length {
            let sequence_length = if let Some(static_size) = T::static_length() {
                static_size
            } else {
                iterator.next().unwrap().value() as usize
            };
            let sequence = (0..sequence_length)
                .map(|_| iterator.next().unwrap())
                .collect_vec();
            let object = *T::decode(&sequence).map_err(|e| e.into())?;
            vector.push(object);
        }
        Ok(Box::new(vector))
    }
}

/// Convenience struct for converting between string literals and field name identifiers.
pub trait TasmObjectFieldName {
    fn tasm_object_field_name(&self) -> String;
}

impl TasmObjectFieldName for &str {
    fn tasm_object_field_name(&self) -> String {
        self.to_string()
    }
}

impl TasmObjectFieldName for i32 {
    fn tasm_object_field_name(&self) -> String {
        format!("field_{}", self)
    }
}

/// Convenience macro, so that we don't have to write
/// ```ignore
/// let field_f = <StructWithNamedFields as TasmObject>::get_field!("f");
/// let field_0 = <StructWithUnnamedFields as TasmObject>::get_field!("field_0");
/// ```
/// but instead
/// ```ignore
/// let field_f = field!(StructWithNamedFields::f);
/// let field_0 = field!(StructWithUnnamedFields::0);
/// ```
/// .
///
/// **Limitations** The type descriptor cannot have generic type arguments. To get around
/// this, define a new type via `type Custom = Generic<T>` and use that instead.
#[macro_export]
macro_rules! field {
    { $o : ident :: $e : ident } => {
        <$o as $crate::structure::tasm_object::TasmObject>
            ::get_field(& $crate::structure::tasm_object::TasmObjectFieldName::tasm_object_field_name(&stringify!($e))
        )
    };
    { $o : ident :: $e : expr } => {
        <$o as $crate::structure::tasm_object::TasmObject>
            ::get_field(& $crate::structure::tasm_object::TasmObjectFieldName::tasm_object_field_name(&$e)
        )
    };
}

/// Convenience macro, so that we don't have to write
/// ```ignore
/// let field_f = <StructWithNamedFields as TasmObject>::get_field_with_size!("f");
/// let field_0 = <StructWithUnnamedFields as TasmObject>::get_field_with_size!("field_0");
/// ```
/// but instead
/// ```ignore
/// let field_f = field_with_size!(StructWithNamedFields::f);
/// let field_0 = field_with_size!(StructWithUnnamedFields::0);
/// ```
/// and for numbered fields.
///
/// **Limitations** The type descriptor cannot have generic type arguments. To get around
/// this, define a new type via `type Custom = Generic<T>` and use that instead.
#[macro_export]
macro_rules! field_with_size {
    { $o : ident :: $e : ident } => {
        <$o as $crate::structure::tasm_object::TasmObject>
            ::get_field_with_size(
                & $crate::structure::tasm_object::TasmObjectFieldName::tasm_object_field_name(&stringify!($e))
            )
    };
    { $o : ident :: $e : expr } => {
        <$o as $crate::structure::tasm_object::TasmObject>
            ::get_field_with_size(
                & $crate::structure::tasm_object::TasmObjectFieldName::tasm_object_field_name(&$e)
            )
    };
}

/// Turns a memory, represented as a `HashMap` from `BFieldElement`s to `BFieldElement`s,
/// along with a starting address, into an iterator over `BFieldElement`s.
pub struct MemoryIter<'a> {
    memory: &'a HashMap<BFieldElement, BFieldElement>,
    address: BFieldElement,
}

impl<'a> MemoryIter<'a> {
    pub fn new(memory: &'a HashMap<BFieldElement, BFieldElement>, address: BFieldElement) -> Self {
        Self { memory, address }
    }
}

impl<'a> Iterator for MemoryIter<'a> {
    type Item = BFieldElement;

    fn next(&mut self) -> Option<Self::Item> {
        let element = self
            .memory
            .get(&self.address)
            .copied()
            .unwrap_or(BFieldElement::zero());
        self.address.increment();
        Some(element)
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use arbitrary::{Arbitrary, Unstructured};
    use itertools::Itertools;
    use rand::RngCore;
    use rand::{rngs::StdRng, thread_rng, Rng, SeedableRng};
    use triton_vm::instruction::LabelledInstruction;
    use triton_vm::{proof_item::FriResponse, triton_asm, BFieldElement, NonDeterminism};
    use twenty_first::shared_math::{bfield_codec::BFieldCodec, x_field_element::XFieldElement};

    use crate::data_type::DataType;
    use crate::memory::encode_to_memory;
    use crate::{
        empty_stack, execute_with_terminal_state, library::Library,
        list::unsafeimplu32::length::Length, structure::tasm_object::TasmObject, Digest,
    };

    #[test]
    fn test_load_and_decode_from_memory() {
        #[derive(Debug, Clone, PartialEq, Eq, BFieldCodec)]
        enum InnerEnum {
            Cow(u32),
            Horse(u128, u128),
            Pig(XFieldElement),
            Sheep([BFieldElement; 13]),
        }

        #[derive(Debug, Clone, PartialEq, Eq, BFieldCodec, TasmObject)]
        struct InnerStruct(XFieldElement, u32);

        #[derive(Debug, Clone, PartialEq, Eq, BFieldCodec, TasmObject)]
        struct OuterStruct {
            o: InnerEnum,
            a: Vec<Option<bool>>,
            b: InnerStruct,
            p: InnerEnum,
            c: BFieldElement,
            l: InnerEnum,
        }

        fn pseudorandom_object(seed: [u8; 32]) -> OuterStruct {
            let mut rng: StdRng = SeedableRng::from_seed(seed);
            let a = (0..19)
                .map(|_| if rng.gen() { Some(rng.gen()) } else { None })
                .collect_vec();
            let b0: XFieldElement = rng.gen();
            let b1: u32 = rng.gen();
            let c: BFieldElement = rng.gen();

            OuterStruct {
                o: InnerEnum::Pig(XFieldElement::new_const(443u64.into())),
                a,
                b: InnerStruct(b0, b1),
                p: InnerEnum::Cow(999),
                c,
                l: InnerEnum::Horse(1 << 99, 1 << 108),
            }
        }

        let mut rng = thread_rng();
        let mut memory: HashMap<BFieldElement, BFieldElement> = HashMap::new();

        let object = pseudorandom_object(rng.gen());
        let address = rng.gen();
        encode_to_memory(&mut memory, address, object.clone());
        let object_again: OuterStruct = *OuterStruct::decode_from_memory(&memory, address).unwrap();
        assert_eq!(object, object_again);
    }

    /// Test derivation of field getters and manual derivations of the `field!` macro
    mod derive_tests {
        use triton_vm::Program;

        use super::*;

        #[test]
        fn load_and_decode_struct_with_named_fields_from_memory() {
            #[derive(BFieldCodec, TasmObject, PartialEq, Eq, Clone, Debug, Arbitrary)]
            struct NamedFields {
                a: Digest,
                b: BFieldElement,
                c: u128,
                d: Vec<Digest>,
                e: XFieldElement,
                f: Vec<u32>,
            }

            let mut randomness = [0u8; 100000];
            thread_rng().fill_bytes(&mut randomness);
            let mut unstructured = Unstructured::new(&randomness);
            let random_object = NamedFields::arbitrary(&mut unstructured).unwrap();
            let random_address: u64 = thread_rng().gen_range(0..(1 << 30));
            let address = random_address.into();
            let mut memory: HashMap<BFieldElement, BFieldElement> = HashMap::new();

            encode_to_memory(&mut memory, address, random_object.clone());
            let object_again: NamedFields =
                *NamedFields::decode_from_memory(&memory, address).unwrap();
            assert_eq!(random_object, object_again);

            let mut library = Library::new();
            let length_d = library.import(Box::new(Length {
                data_type: DataType::Digest,
            }));
            let length_f = library.import(Box::new(Length {
                data_type: DataType::U32,
            }));
            let code = triton_asm! {
                    // _ *obj
                    dup 0 {&field!(NamedFields::d)}

                    // _ *obj *d
                    swap 1
                   {&field!(NamedFields::f)}
                    // _ *d *f

                    call {length_f}
                    // _ *d f_length

                    swap 1
                    call {length_d}
                    // _ f_length d_length
            };

            let mut stack = get_final_stack(&random_object, library, code);
            let extracted_d_length = stack.pop().unwrap().value() as usize;
            let extracted_f_length = stack.pop().unwrap().value() as usize;

            assert_eq!(random_object.d.len(), extracted_d_length);
            assert_eq!(random_object.f.len(), extracted_f_length);
        }

        #[test]
        fn load_and_decode_tuple_struct_containing_enums_from_memory() {
            #[derive(BFieldCodec, PartialEq, Eq, Clone, Debug, Arbitrary)]
            enum MyEnum {
                A(u64, Digest),
                B,
                C,
            }

            #[derive(BFieldCodec, TasmObject, PartialEq, Eq, Clone, Debug, Arbitrary)]
            struct TupleStruct(
                Vec<XFieldElement>,
                MyEnum,
                u32,
                Vec<Digest>,
                Digest,
                Vec<BFieldElement>,
                Digest,
            );

            let mut randomness = [0u8; 100000];
            thread_rng().fill_bytes(&mut randomness);
            let mut unstructured = Unstructured::new(&randomness);
            let random_object = TupleStruct::arbitrary(&mut unstructured).unwrap();
            let mut memory: HashMap<BFieldElement, BFieldElement> = HashMap::new();
            let random_address: u64 = thread_rng().gen_range(0..(1 << 30));
            let address = random_address.into();

            encode_to_memory(&mut memory, address, random_object.clone());
            let object_again: TupleStruct =
                *TupleStruct::decode_from_memory(&memory, address).unwrap();
            assert_eq!(random_object, object_again);

            // code snippet to access object's fields
            let mut library = Library::new();
            let length_digests = library.import(Box::new(Length {
                data_type: DataType::Digest,
            }));
            let length_bfes = library.import(Box::new(Length {
                data_type: DataType::Bfe,
            }));
            let length_xfes = library.import(Box::new(Length {
                data_type: DataType::Xfe,
            }));
            let code = triton_asm! {
                // _ *obj

                dup 0
                {&field!(TupleStruct::3)} // _ *obj *digests
                swap 1                    // _ *digests *obj

                dup 0
                {&field!(TupleStruct::5)} // _ *digests *obj *bfes
                swap 1                    // _ *digests *bfes *obj

                {&field!(TupleStruct::0)} // _ *digests *bfes *xfes
                call {length_xfes}     // _ *digests *bfes xfe_count
                swap 2                 // _ xfe_count *bfes *digests
                call {length_digests}  // _ xfe_count *bfes digest_count
                swap 1
                call {length_bfes}     // _ xfe_count digest_count bfe_count
            };

            // extract list lengths
            let mut stack = get_final_stack(&random_object, library, code);
            let extracted_bfe_count = stack.pop().unwrap().value() as usize;
            let extracted_digest_count = stack.pop().unwrap().value() as usize;
            let extracted_xfe_count = stack.pop().unwrap().value() as usize;

            // assert correct lengths
            assert_eq!(random_object.3.len(), extracted_digest_count);
            assert_eq!(random_object.5.len(), extracted_bfe_count);
            assert_eq!(random_object.0.len(), extracted_xfe_count);
        }

        #[test]
        fn test_fri_response() {
            let mut rng = thread_rng();
            let num_digests = 50;
            let num_leafs = 20;

            // generate object
            let authentication_structure =
                (0..num_digests).map(|_| rng.gen::<Digest>()).collect_vec();
            let revealed_leafs = (0..num_leafs)
                .map(|_| rng.gen::<XFieldElement>())
                .collect_vec();
            let fri_response = FriResponse {
                auth_structure: authentication_structure,
                revealed_leaves: revealed_leafs,
            };

            // code snippet to access object's fields
            let mut library = Library::new();
            let get_authentication_structure = field!(FriResponse::auth_structure);
            let length_digests = library.import(Box::new(Length {
                data_type: DataType::Digest,
            }));
            let get_revealed_leafs = field!(FriResponse::revealed_leaves);
            let length_xfes = library.import(Box::new(Length {
                data_type: DataType::Xfe,
            }));
            let code = triton_asm! {
                // _ *fri_response
                dup 0 // _ *fri_response *fri_response

                {&get_authentication_structure} // _ *fri_response *authentication_structure
                swap 1                          // _ *authentication_structure *fri_response
                {&get_revealed_leafs}           // _ *authentication_structure *revealed_leafs

                swap 1                          // _ *revealed_leafs *authentication_structure
                call {length_digests}           // _ *revealed_leafs num_digests
                swap 1                          // _ num_digests *revealed_leafs
                call {length_xfes}              // _ num_digests num_leafs

            };

            // extract list lengths
            let mut stack = get_final_stack(&fri_response, library, code);
            let extracted_xfes_length = stack.pop().unwrap().value() as usize;
            let extracted_digests_length = stack.pop().unwrap().value() as usize;

            // assert correct lengths
            assert_eq!(num_digests, extracted_digests_length);
            assert_eq!(num_leafs, extracted_xfes_length);
        }

        /// Helper function for testing field getters. Only returns the final stack.
        fn get_final_stack<T: BFieldCodec + Clone>(
            obj: &T,
            library: Library,
            code: Vec<LabelledInstruction>,
        ) -> Vec<BFieldElement> {
            // initialize memory and stack
            let mut memory: HashMap<BFieldElement, BFieldElement> = HashMap::new();
            let random_address: u64 = thread_rng().gen_range(0..(1 << 30));
            let address = random_address.into();

            encode_to_memory(&mut memory, address, obj.to_owned());
            let stack = [empty_stack(), vec![address]].concat();

            // link by hand
            let entrypoint = "entrypoint";
            let library_code = library.all_imports();
            let instructions = triton_asm!(
                call {entrypoint}
                halt

                {entrypoint}:
                    {&code}
                    return

                {&library_code}
            );

            let program = Program::new(&instructions);
            let nondeterminism = NonDeterminism::new(vec![]).with_ram(memory);
            let final_state =
                execute_with_terminal_state(&program, &[], &stack, &nondeterminism, None).unwrap();
            final_state.op_stack.stack
        }
    }
}

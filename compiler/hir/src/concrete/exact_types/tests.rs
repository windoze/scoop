use la_arena::Arena;

use super::*;
use crate::IntegerTypeCore;

#[test]
fn empty_tuple_cannot_enter_the_exact_type_relation() {
    let mut types = Arena::new();
    let tuple = types.alloc(Type {
        kind: TypeKind::Tuple(Vec::new()),
        gc_free: true,
    });
    let function_types = Arena::new();
    let structs = Arena::new();
    let enums = Arena::new();
    let classes = Arena::new();
    let interfaces = Arena::new();
    let objects = Arena::new();
    let intrinsic_core = IntrinsicTypeCore {
        integers: IntegerTypeCore::new(std::array::from_fn(|index| {
            StructId::from_raw((index as u32).into())
        }))
        .expect("the synthetic integer owner ids are distinct"),
        boolean: StructId::from_raw(0_u32.into()),
        string: ClassId::from_raw(0_u32.into()),
    };

    let error = ExactTypeIdentities::from_types(ExactTypeIdentityInputs {
        types: &types,
        function_types: &function_types,
        structs: &structs,
        enums: &enums,
        classes: &classes,
        interfaces: &interfaces,
        objects: &objects,
        core_types: ConcreteCoreTypeIdentityAuthority::Defined(&intrinsic_core),
    })
    .expect_err("an empty tuple has no exact structural identity");
    assert!(matches!(
        error,
        ExactTypeIdentityError::EmptyTuple { ty } if ty == tuple.into_raw().into_u32()
    ));
}

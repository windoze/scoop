//! Machine-only storage types used by emission.

use scoop_lir::{
    EnumDefId, EnumDefs, EnumRepr, LirType, StructDefId, StructDefs, StructRepresentation,
};
use std::collections::HashSet;

pub(crate) fn contains_machine_scalar(
    structs: &StructDefs,
    enums: &EnumDefs,
    ty: &LirType,
) -> bool {
    fn visit(
        structs: &StructDefs,
        enums: &EnumDefs,
        ty: &LirType,
        seen_structs: &mut HashSet<StructDefId>,
        seen_enums: &mut HashSet<EnumDefId>,
    ) -> bool {
        match ty {
            LirType::MachineScalar(_) => true,
            LirType::Aggregate(elements) => elements
                .iter()
                .any(|element| visit(structs, enums, element, seen_structs, seen_enums)),
            LirType::Struct(id) if seen_structs.insert(*id) => {
                let definition = &structs[*id];
                if let StructRepresentation::Intrinsic(
                    scoop_lir::IntrinsicTypeRepresentation::MaybeUninit { value, .. },
                ) = &definition.representation
                {
                    return visit(structs, enums, value, seen_structs, seen_enums);
                }
                (0..definition.field_count()).any(|index| {
                    let field_type = definition
                        .field_storage_type(index)
                        .expect("index is below the field count");
                    visit(structs, enums, &field_type, seen_structs, seen_enums)
                })
            }
            LirType::Enum(id) if seen_enums.insert(*id) => match &enums[*id].repr {
                EnumRepr::Niche { .. } | EnumRepr::Integral { .. } => false,
                EnumRepr::Tagged { variants, .. } => variants.iter().any(|variant| {
                    variant
                        .fields
                        .iter()
                        .any(|field| visit(structs, enums, &field.ty, seen_structs, seen_enums))
                }),
            },
            _ => false,
        }
    }

    visit(structs, enums, ty, &mut HashSet::new(), &mut HashSet::new())
}

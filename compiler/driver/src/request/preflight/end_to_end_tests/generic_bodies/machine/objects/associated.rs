use super::*;
use scoop_identity::DefinitionAtomRole;

pub(in super::super) fn change_associated_atom(
    emitted: &scoop_codegen::EmittedConeObjectSetV2,
    role: DefinitionAtomRole,
) -> PersistentCallableBodyId {
    let normalization = emitted.target().contract().native_symbol_normalization();
    for member in emitted.members() {
        let scoop_codegen::EmittedConeObjectMemberKindV1::CallableBody { body, .. } = member.kind()
        else {
            continue;
        };
        let definition = member
            .units()
            .definition_plans()
            .iter()
            .filter_map(|id| emitted.production().canonical_definitions().plan(*id))
            .find(|plan| {
                plan.definition_role() == scoop_identity::StrongDefinitionRole::CallableBody
            })
            .unwrap();
        if definition.primary_symbol().linkage() != LinkageClass::OdrWeak {
            continue;
        }
        let Some(boundary) = definition
            .atom_boundaries()
            .iter()
            .find(|boundary| boundary.atom_role() == role)
        else {
            continue;
        };
        let mut bytes = std::fs::read(member.path()).unwrap();
        let object = object::File::parse(bytes.as_slice()).unwrap();
        let symbol = |request: scoop_identity::PersistentSymbolRequest| {
            object
                .symbol_by_name(
                    &normalization.compiler_generated_object_symbol(request.symbol().as_str()),
                )
                .unwrap()
        };
        let start = symbol(boundary.start());
        let end = symbol(boundary.end());
        let section = object
            .section_by_index(start.section_index().unwrap())
            .unwrap();
        let range = start.address() - section.address()..end.address() - section.address();
        let relocations = section.relocations().collect::<Vec<_>>();
        let file_start = section.file_range().unwrap().0;
        if role == DefinitionAtomRole::Stackmap {
            let start = usize::try_from(file_start + range.start).unwrap();
            assert_eq!(
                u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap()),
                1
            );
            let stack_size = u64::from_le_bytes(bytes[start + 24..start + 32].try_into().unwrap());
            bytes[start + 24..start + 32]
                .copy_from_slice(&stack_size.checked_add(16).unwrap().to_le_bytes());
        } else {
            let offset = range
                .rev()
                .find(|offset| {
                    relocations.iter().all(|(start, relocation)| {
                        !(*start..*start + u64::from(relocation.size().div_ceil(8)))
                            .contains(offset)
                    })
                })
                .expect("associated atom contains non-relocated data");
            let offset = usize::try_from(file_start + offset).unwrap();
            bytes[offset] ^= 1;
        }
        let replacement = member.path().with_extension("changed");
        std::fs::write(&replacement, bytes).unwrap();
        std::fs::rename(replacement, member.path()).unwrap();
        return body;
    }
    panic!("fixture has no ODR callable with {role:?}");
}

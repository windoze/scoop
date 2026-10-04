//! Persistent type fixtures carry the provider used by their source keys.

use super::*;

pub(in crate::tests) fn test_exact_type(
    function_types: &Arena<mir::FunctionType>,
    ty: &mir::Type,
    entries: &mut Vec<mir::SourceExactTypeIdentity>,
) -> PersistentExactTypeId {
    test_exact_type_at(ConeIdentity::SINGLE_FILE, function_types, ty, entries)
}

pub(super) fn test_tuple_layout_type(
    provider: ConeIdentity,
    function_types: &Arena<mir::FunctionType>,
    ty: &mir::Type,
    entries: &mut Vec<mir::SourceExactTypeIdentity>,
) {
    if matches!(ty, mir::Type::Tuple(_)) {
        test_exact_type_at(provider, function_types, ty, entries);
    }
}

pub(super) fn test_declaration_site(provider: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

pub(super) fn test_exact_type_at(
    provider: ConeIdentity,
    function_types: &Arena<mir::FunctionType>,
    ty: &mir::Type,
    entries: &mut Vec<mir::SourceExactTypeIdentity>,
) -> PersistentExactTypeId {
    if let Some(existing) = entries.iter().find(|entry| entry.ty() == ty) {
        return existing.identity_record().id();
    }
    let key = match ty {
        mir::Type::Context(storage) => storage.exact_record().key().clone(),
        mir::Type::Unit => ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        mir::Type::Any => ExactTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id()),
        mir::Type::Integer(kind) => test_nominal_exact(
            provider,
            &format!("Test{}", kind.canonical_name()),
            SourceNominalKind::Struct,
        ),
        mir::Type::MachineScalar(kind) => test_nominal_exact(
            provider,
            &format!(
                "TestMachine{}",
                kind.name()
                    .bytes()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ),
            SourceNominalKind::Struct,
        ),
        mir::Type::Boolean => {
            test_nominal_exact(provider, "TestBoolean", SourceNominalKind::Struct)
        }
        mir::Type::String => test_nominal_exact(provider, "TestString", SourceNominalKind::Class),
        mir::Type::Struct(id) => test_nominal_exact(
            provider,
            &format!("TestStruct{}", id.into_raw().into_u32()),
            SourceNominalKind::Struct,
        ),
        mir::Type::Class(id) => test_nominal_exact(
            provider,
            &format!("TestClass{}", id.into_raw().into_u32()),
            SourceNominalKind::Class,
        ),
        mir::Type::Interface(id) => test_nominal_exact(
            provider,
            &format!("TestInterface{}", id.into_raw().into_u32()),
            SourceNominalKind::Interface,
        ),
        mir::Type::Enum(id, _) => test_nominal_exact(
            provider,
            &format!("TestEnum{}", id.into_raw().into_u32()),
            SourceNominalKind::Enum,
        ),
        mir::Type::Tuple(elements) => ExactTypeKey::Tuple(
            scoop_identity::NonEmptyVec::new(
                elements
                    .iter()
                    .map(|element| test_exact_type_at(provider, function_types, element, entries))
                    .collect(),
            )
            .expect("test tuple types are non-empty"),
        ),
        mir::Type::Function(id) => {
            let signature = function_types[*id].clone();
            ExactTypeKey::Function {
                effect: if signature.is_suspend {
                    Effect::Suspend
                } else {
                    Effect::Ordinary
                },
                parameters: signature
                    .parameter_types
                    .iter()
                    .map(|parameter| {
                        test_exact_type_at(provider, function_types, parameter, entries)
                    })
                    .collect(),
                result: test_exact_type_at(
                    provider,
                    function_types,
                    &signature.return_type,
                    entries,
                ),
            }
        }
        mir::Type::Ptr(pointee) => ExactTypeKey::RawPointer(test_exact_type_at(
            provider,
            function_types,
            pointee,
            entries,
        )),
        mir::Type::FunPtr(id) => {
            let signature = function_types[*id].clone();
            ExactTypeKey::NativeFunctionPointer {
                calling_convention: scoop_identity::CallingConvention::C,
                parameters: signature
                    .parameter_types
                    .iter()
                    .map(|parameter| {
                        test_exact_type_at(provider, function_types, parameter, entries)
                    })
                    .collect(),
                result: test_exact_type_at(
                    provider,
                    function_types,
                    &signature.return_type,
                    entries,
                ),
            }
        }
    };
    let origin = match &key {
        ExactTypeKey::Nominal(_) => {
            mir::SourceExactTypeOrigin::Nominal(if matches!(ty, mir::Type::Unit | mir::Type::Any) {
                ConeIdentity::CORE
            } else {
                provider
            })
        }
        ExactTypeKey::NominalApplication { .. } => {
            unreachable!("test applications have explicit groups")
        }
        ExactTypeKey::Tuple(_)
        | ExactTypeKey::Function { .. }
        | ExactTypeKey::RawPointer(_)
        | ExactTypeKey::NativeFunctionPointer { .. } => mir::SourceExactTypeOrigin::Structural,
    };
    let record = CborIdentityRecord::from_key(key).unwrap();
    let id = record.id();
    entries.push(mir::SourceExactTypeIdentity::checked(ty.clone(), record, origin).unwrap());
    id
}

fn test_nominal_exact(provider: ConeIdentity, name: &str, kind: SourceNominalKind) -> ExactTypeKey {
    let declaration = SourceDeclarationKey::nominal(
        test_declaration_site(provider),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    );
    ExactTypeKey::Nominal(PersistentTypeId::from_source_declaration(&declaration).unwrap())
}

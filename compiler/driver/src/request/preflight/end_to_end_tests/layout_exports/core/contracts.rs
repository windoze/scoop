use super::*;
use scoop_identity::{CallableTemplateOrigin, StrongCallableDefinitionOwner};
use scoop_lir_lower::LayoutAbiExportInputV1;

mod descriptors;

pub(super) fn check(
    name: &str,
    hir: &current_hir::CurrentConeHirArtifacts,
    source: &hir::CrossConeTypeSemanticsSectionV1,
    input: LayoutAbiExportInputV1<'_>,
    result: &lir::LayoutAbiExportConstituentsV1,
) {
    for ty in [mir::Type::Unit, mir::Type::Any] {
        let exact = input
            .mir
            .module()
            .meta
            .source_exact_types
            .get(&ty)
            .unwrap()
            .identity_record()
            .id();
        let fact = source.exact_facts().get(exact).unwrap();
        let shape = input.bridge.types().get(exact).unwrap();
        assert_eq!(
            shape.facts().kind(),
            if ty == mir::Type::Unit {
                mir::MirValueKindV1::ZeroSizedValue
            } else {
                mir::MirValueKindV1::Reference
            }
        );
        assert!(result.descriptors().get(exact).is_some());
        assert_eq!(
            (fact.kind(), fact.gc()),
            if ty == mir::Type::Unit {
                (
                    hir::ExactTypeKindV1::Value {
                        zst: hir::ZstStatus::ZeroSized,
                    },
                    hir::ExactTypeGcV1::GcFree,
                )
            } else {
                (
                    hir::ExactTypeKindV1::Reference,
                    hir::ExactTypeGcV1::ContainsManagedReferences,
                )
            }
        );
    }
    source_only_callable(hir, input);
    reject_missing_string_vtable(input);
    let (names, local_only) = descriptors::check(&hir.cross_cone_section, input, result);
    let previously_deferred: &[&str] = match name {
        "shared-callables-combined" => &["SharedSourceOnlyHolder"],
        "shared-ordinary-combined" => &["SharedOrdinaryDeferred", "SharedOrdinaryDeferredValue"],
        _ => &[],
    };
    for name in [
        "ArithmeticException",
        "ClassCastException",
        "Exception",
        "IllegalStateException",
        "IndexOutOfBoundsException",
        "UnwrapException",
    ]
    .into_iter()
    .chain(previously_deferred.iter().copied())
    {
        let local = input
            .mir
            .module()
            .meta
            .source_exact_types
            .iter()
            .find(|local| mir::type_name(input.mir.module(), local.ty()) == name)
            .unwrap();
        let exact = local.identity_record().id();
        assert!(input.bridge.types().get(exact).is_some());
        assert!(result.descriptors().get(exact).is_some());
    }
    assert!(
        names.is_empty(),
        "unexpected source-only descriptors: {names:?}"
    );
    // Check representative body-only shapes and this fixture's private type.
    // Every descriptor's publication and ABI contract is checked above.
    let private_probe = match name {
        "shared-callables-combined" => Some("SharedCallableImpl"),
        "shared-equality-standalone" => Some("SharedEqualityHidden"),
        "shared-units-combined" => Some("SharedUnitHidden"),
        _ => None,
    };
    for expected in [
        "(Long) -> String",
        "closure (Long) -> String",
        "StringIterator",
    ]
    .into_iter()
    .chain(private_probe)
    {
        assert!(
            local_only.iter().any(|actual| actual == expected),
            "missing local-only descriptor: {expected}"
        );
    }
}

fn source_only_callable(
    hir: &current_hir::CurrentConeHirArtifacts,
    input: LayoutAbiExportInputV1<'_>,
) {
    let export = hir.hir.output().export.module();
    let (function, _) = export
        .functions
        .iter()
        .find(|(_, function)| function.name == "UInt16.rangeTo")
        .unwrap();
    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(identity)) =
        &export.function_identities[function]
    else {
        panic!("the range declaration is a plain source function")
    };
    assert!(
        hir.cross_cone_section
            .callable_interfaces()
            .records()
            .iter()
            .any(|callable| {
                callable.declaration() == CallableTemplateOrigin::Function(identity.id())
            })
    );
    assert!(input.bridge.callables().entries().iter().all(|callable| {
        callable.implementation()
            != scoop_identity::CallableDefinitionOwner::Strong(
                StrongCallableDefinitionOwner::Function(identity.id()),
            )
    }));
}

fn reject_missing_string_vtable(input: LayoutAbiExportInputV1<'_>) {
    let exact = input
        .mir
        .module()
        .meta
        .source_exact_types
        .get(&mir::Type::String)
        .unwrap()
        .identity_record()
        .id();
    let schema = input.bridge.dispatch().get(exact).unwrap();
    assert!(matches!(
        schema.slots(),
        mir::MirDispatchSlotsV1::ClassVtable(_)
    ));
    assert!(matches!(mir::ParamFreeMirDispatchSchemaV1::try_new(
        mir::MirDispatchSchemaAuthority {
            identities: input.identities,
            types: input.bridge.types(),
            callables: input.bridge.callables(),
        },
        exact,
        mir::MirDispatchSlotsV1::NoClassVtable,
        schema.itables().to_vec(),

    ), Err(mir::MirDispatchSchemaError::OwnerKind { owner }) if owner == exact));
}

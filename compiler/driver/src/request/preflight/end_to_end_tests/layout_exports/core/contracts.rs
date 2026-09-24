use super::*;
use scoop_identity::{CallableTemplateOrigin, ExactTypeKey, StrongCallableDefinitionOwner};
use scoop_lir_lower::LayoutAbiExportInputV1;

pub(super) fn check(
    name: &str,
    hir: &current_hir::CurrentConeHirArtifacts,
    source: &hir::CrossConeTypeSemanticsProductionV1,
    input: LayoutAbiExportInputV1<'_>,
    result: &lir::LayoutAbiExportConstituentsV1,
) -> String {
    let mut dump = String::new();
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
        let fact = source.section().exact_facts().get(exact).unwrap();
        assert!(source.fact_shape(exact).is_some());
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
        dump.push_str(&format!(
            "hir {ty:?}: {:?} {:?}\nmir {ty:?}: {:?} {:?}\n",
            fact.kind(),
            fact.gc(),
            shape.facts(),
            shape.representation(),
        ));
    }
    source_only_callable(hir, input);
    reject_missing_string_vtable(input);
    let (names, local_only) = source_only_descriptors(&hir.cross_cone_section, input, result);
    let mut expected = vec![
        "ArithmeticException",
        "ClassCastException",
        "Exception",
        "IllegalStateException",
        "IndexOutOfBoundsException",
        "UnwrapException",
    ];
    if name == "shared-callables-combined" {
        expected.push("SharedSourceOnlyHolder");
        expected.sort();
    }
    assert_eq!(names, expected);
    assert_eq!(
        local_only,
        if name == "shared-callables-combined" {
            vec!["SharedCallableImpl"]
        } else {
            vec![]
        }
    );
    for name in names {
        dump.push_str(&format!("source-only descriptor {name}\n"));
    }
    for name in local_only {
        dump.push_str(&format!("local-only descriptor {name}\n"));
    }
    dump
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
        callable.implementation() != StrongCallableDefinitionOwner::Function(identity.id())
    }));
}

fn source_only_descriptors(
    public: &hir::CrossConeHirInterfaceSectionV1,
    input: LayoutAbiExportInputV1<'_>,
    result: &lir::LayoutAbiExportConstituentsV1,
) -> (Vec<String>, Vec<String>) {
    let closure = hir::NominalMaterializationClosure::from_declarations(
        public.nominal_interfaces(),
        public.callable_interfaces(),
        &mut meter(),
    )
    .unwrap();
    let mut names = Vec::new();
    let mut local_only = Vec::new();
    let mut exported = 0;
    for (_, descriptor) in input.lir.module().meta.type_descriptors.iter() {
        let exact = descriptor.identity.exact_type();
        let local = input
            .mir
            .module()
            .meta
            .source_exact_types
            .get_by_identity(exact);
        if let Some(local) = local {
            if let (ExactTypeKey::Nominal(nominal), mir::Type::Class(class)) =
                (local.identity_record().key(), local.ty())
            {
                if public
                    .nominal_interfaces()
                    .declaration(hir::SourceNominalId::Concrete(*nominal))
                    .is_none()
                {
                    assert!(input.bridge.types().get(exact).is_none());
                    assert!(result.descriptors().get(exact).is_none());
                    local_only.push(input.mir.module().classes[*class].name.clone());
                    continue;
                }
            }
        }
        let source_only = local.is_some_and(|local| {
            let ExactTypeKey::Nominal(nominal) = local.identity_record().key() else {
                return false;
            };
            public
                .nominal_interfaces()
                .declaration(hir::SourceNominalId::Concrete(*nominal))
                .is_some()
                && !closure.contains(*nominal)
        });
        if source_only {
            assert!(input.bridge.types().get(exact).is_none());
            assert!(result.descriptors().get(exact).is_none());
            let mir::Type::Class(class) = local.unwrap().ty() else {
                panic!("the actual core's unexported local types are exception classes")
            };
            names.push(input.mir.module().classes[*class].name.clone());
        } else {
            assert!(
                result.descriptors().get(exact).is_some(),
                "missing {exact}: {local:?}"
            );
            exported += 1;
        }
    }
    assert_eq!(result.descriptors().records().len(), exported);
    names.sort();
    local_only.sort();
    (names, local_only)
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
        schema.vtable(),
        mir::MirClassVtableSchemaV1::ClassVtable(_)
    ));
    assert!(matches!(mir::ParamFreeMirDispatchSchemaV1::try_new(
        mir::MirDispatchSchemaAuthority {
            identities: input.identities,
            types: input.bridge.types(),
            callables: input.bridge.callables(),
        },
        exact,
        mir::MirClassVtableSchemaV1::NoClassVtable,
        schema.itables().to_vec(),
        &mut meter(),
    ), Err(mir::MirDispatchSchemaError::OwnerKind { owner }) if owner == exact));
}

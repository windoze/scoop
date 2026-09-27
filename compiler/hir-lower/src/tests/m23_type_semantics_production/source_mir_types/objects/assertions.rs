use super::*;
use scoop_mir::{MirCallableLoweringRoleV1 as Role, MirTypeBridgeTypeLookupV1};
use std::fmt::Write;

pub(super) fn actual(
    output: &hir::DependencyHirOutput,
    input: &scoop_mir::ConeMirInput,
    product: &Production,
) {
    let export = output.output().export.module();
    let local = output.output().local.module();
    let module = input.module();
    for (_, object) in export.objects.iter() {
        let identity = export.object_value_identities[object.singleton_value].id();
        assert_eq!(
            local
                .singleton_values
                .iter()
                .filter(|(_, value)| value.identity == identity)
                .count(),
            1
        );
        let (_, value) = module
            .singleton_values
            .iter()
            .find(|(_, value)| value.identity == identity)
            .unwrap();
        let Some(record) = product.objects().get(identity) else {
            continue;
        };
        let root = input
            .materialization()
            .initialization_roots()
            .iter()
            .find(|root| root.unit() == value.initialization)
            .unwrap();
        assert_eq!(record.unit(), root.identity());
        let global = &module.globals[module.singleton_published_roots[value.published_root].global];
        assert_eq!(
            record.read().object(),
            module
                .meta
                .source_exact_types
                .get(&global.ty)
                .unwrap()
                .identity_record()
                .id()
        );
        for (actual, role) in [
            (
                root.initializer(),
                Role::ObjectInitializer {
                    unit: root.identity(),
                },
            ),
            (
                root.ensure(),
                Role::ObjectEnsure {
                    unit: root.identity(),
                },
            ),
        ] {
            let scoop_mir::CallableSignatureSubject::Strong(scoop_mir::CallableOwner::Generated(
                id,
            )) = actual.subject()
            else {
                panic!("generated initialization")
            };
            let owner = scoop_identity::StrongCallableDefinitionOwner::GeneratedCallable(id);
            let binding = product.callables().get(owner).unwrap();
            assert_eq!(*binding.lowering_role(), role);
            assert_eq!(
                binding.lowered_signature().gc_effect(),
                module.functions[actual.function()].gc_effect
            );
            assert_eq!(binding.semantic_signature(), binding.lowered_signature());
            assert_eq!(
                binding.lowered_signature().exact(),
                module
                    .meta
                    .callable_signatures
                    .get(actual.subject())
                    .unwrap()
                    .signature()
            );
            if matches!(role, Role::ObjectEnsure { .. }) {
                assert_eq!(record.ensure(), owner);
            }
        }
    }
    assert_eq!(
        product.callables().entries().len(),
        product.objects().records().len() * 2
    );
}

pub(super) fn wire(
    input: &scoop_mir::ConeMirInput,
    graph: &mut scoop_identity::ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    product: &Production,
) {
    let callables: scoop_mir::DecodedCanonicalMirCallableBindingsV1 = decoded(product.callables());
    let callables = callables
        .validate(graph, input.foundation(), types)
        .unwrap();
    assert_eq!(&callables, product.callables());
    let objects: scoop_mir::DecodedCanonicalMirObjectValuesV1 = decoded(product.objects());
    assert_eq!(
        objects.validate(graph, types, &callables).unwrap(),
        *product.objects()
    );
}

pub(super) fn projection(output: &hir::DependencyHirOutput, product: &Production) -> String {
    let export = output.output().export.module();
    let mut rows = Vec::new();
    for (_, object) in export.objects.iter() {
        let Some(record) = product
            .objects()
            .get(export.object_value_identities[object.singleton_value].id())
        else {
            continue;
        };
        let ensure = product.callables().get(record.ensure()).unwrap();
        rows.push(format!(
            "{}: {} initializer+ensure={:?} published-root\n",
            object.name,
            match object.kind {
                hir::ObjectKind::Standalone => "standalone",
                hir::ObjectKind::Companion(_) => "companion",
            },
            ensure.lowered_signature().gc_effect()
        ));
    }
    rows.sort();
    let mut text = String::new();
    for row in rows {
        write!(text, "{row}").unwrap();
    }
    text
}

pub(super) fn private_and_property(input: &scoop_mir::ConeMirInput, product: &Production) {
    let module = input.module();
    let (_, hidden) = module
        .objects
        .iter()
        .find(|(_, object)| object.name == "Hidden")
        .unwrap();
    assert!(
        product
            .objects()
            .get(module.singleton_values[hidden.singleton_value].identity)
            .is_none()
    );
    for root in input.materialization().initialization_roots() {
        if matches!(
            module.initialization_units[root.unit()].kind,
            scoop_mir::InitializationUnitKind::EagerTopLevel { .. }
        ) {
            assert!(
                product
                    .objects()
                    .records()
                    .iter()
                    .all(|record| record.unit() != root.identity())
            );
            assert!(product.callables().entries().iter().all(|binding| !matches!(binding.lowering_role(), Role::ObjectEnsure { unit } | Role::ObjectInitializer { unit } if *unit == root.identity())));
        }
    }
}

use super::*;
use scoop_identity::{CallableOwner, GeneratedCallableKey, StrongCallableDefinitionOwner};

pub(super) fn actual(
    input: &ConeMirInput,
    types: &CanonicalParamFreeMirTypeExportsV1,
    source: &CanonicalMirCallableBindingsV1,
    bindings: &CanonicalMirCallableBindingsV1,
) {
    for adjust in &input.module().meta.boxing_adjusts {
        let GeneratedCallableKey::BoxingAdjust { payload, .. } =
            adjust.identity().callable_record().key()
        else {
            panic!("boxing key")
        };
        let id = StrongCallableDefinitionOwner::GeneratedCallable(
            adjust.identity().callable_record().id(),
        );
        if types.get(*payload).is_none() {
            assert!(bindings.get(id).is_none());
            continue;
        }
        let binding = bindings.get(id).unwrap();
        let scoop_mir::MirCallableLoweringRoleV1::BoxingAdjust { target } = binding.lowering_role()
        else {
            panic!("boxing role")
        };
        let root = input
            .materialization()
            .callable_roots()
            .iter()
            .find(|root| scoop_mir::BoxingAdjustTarget::Local(root.function()) == adjust.target())
            .unwrap();
        assert_eq!(
            root.subject(),
            scoop_mir::CallableSignatureSubject::Strong(
                target.strong_owner().unwrap().callable_owner()
            )
        );
        let calls: Vec<_> = input.module().functions[adjust.function()]
            .body
            .blocks
            .iter()
            .flat_map(|(_, block)| &block.statements)
            .filter_map(|statement| match &statement.kind {
                scoop_mir::StatementKind::Call(
                    scoop_mir::CallEffect::Unit(call) | scoop_mir::CallEffect::Value { call, .. },
                ) => Some(call),
                _ => None,
            })
            .collect();
        assert_eq!(calls.len(), 1);
        assert!(matches!(calls[0].target.kind, scoop_mir::CallKind::Direct));
        assert!(
            matches!(calls[0].target.callee, scoop_mir::Callee::User(actual) if scoop_mir::BoxingAdjustTarget::Local(actual) == adjust.target())
        );
        assert_eq!(
            binding.semantic_signature(),
            source.get(*target).unwrap().semantic_signature()
        );
        assert_eq!(
            binding.lowered_signature().exact(),
            adjust.identity().signature_record().signature()
        );
        assert_eq!(
            binding.lowered_signature().gc_effect(),
            input.module().functions[adjust.function()].gc_effect
        );
        assert!(
            input
                .materialization()
                .callable_roots()
                .iter()
                .any(|root| root.function() == adjust.function()
                    && root.subject()
                        == scoop_mir::CallableSignatureSubject::Strong(CallableOwner::Generated(
                            adjust.identity().callable_record().id()
                        )))
        );
        assert!(
            matches!(input.module().classes[adjust.boxed()].itables.iter().find(|table| table.interface == adjust.interface()).unwrap().slots[adjust.slot() as usize], scoop_mir::TableSlot::Function(function) if function == adjust.function())
        );
    }
}

pub(super) fn selection(
    output: &hir::DependencyHirOutput,
    input: &ConeMirInput,
    bindings: &CanonicalMirCallableBindingsV1,
    case: &str,
) {
    let mut payloads = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for adjust in &input.module().meta.boxing_adjusts {
        let id = StrongCallableDefinitionOwner::GeneratedCallable(
            adjust.identity().callable_record().id(),
        );
        let Some(binding) = bindings.get(id) else {
            continue;
        };
        let GeneratedCallableKey::BoxingAdjust { payload, .. } =
            adjust.identity().callable_record().key()
        else {
            panic!("boxing key")
        };
        payloads.insert(*payload);
        let scoop_mir::BoxingAdjustTarget::Local(target) = adjust.target() else {
            panic!("local source target")
        };
        targets.insert(input.module().functions[target].name.as_str());
        assert_eq!(
            binding.semantic_signature().gc_effect(),
            scoop_mir::GcEffect::Managed
        );
        assert_eq!(
            binding.lowered_signature().gc_effect(),
            scoop_mir::GcEffect::Managed
        );
    }
    let owners = source_dispatch::owners(output);
    if case == "standalone" {
        assert_eq!(bindings.entries().len(), 2);
        assert_eq!(payloads, BTreeSet::from([owners["Token"]]));
        assert!(input.module().meta.boxing_adjusts.len() > bindings.entries().len());
    } else {
        assert_eq!(bindings.entries().len(), 24);
        assert_eq!(
            payloads,
            BTreeSet::from([owners["Choice"], owners["Payload"]])
        );
        assert!(targets.contains("Diamond.echo"));
        assert!(targets.contains("Root.$get$token"));
    }
}

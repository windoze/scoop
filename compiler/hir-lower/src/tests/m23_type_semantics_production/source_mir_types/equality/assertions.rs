use super::*;
use scoop_identity::{Effect, GeneratedCallableKey};

pub(super) fn actual(input: &ConeMirInput, bindings: &CanonicalMirCallableBindingsV1) {
    for binding in bindings.entries() {
        let scoop_mir::MirCallableLoweringRoleV1::DerivedEquality { owner } =
            *binding.lowering_role()
        else {
            panic!("derived equality role")
        };
        let scoop_mir::MirCallableOriginV1::Generated {
            role: GeneratedCallableKey::DerivedEquality { exact_owner },
            ..
        } = binding.origin()
        else {
            panic!("derived equality key")
        };
        assert_eq!(owner, *exact_owner);
        assert_eq!(binding.semantic_signature(), binding.lowered_signature());
        let signature = binding.lowered_signature().exact();
        assert_eq!(signature.effect(), Effect::Ordinary);
        assert_eq!(signature.receiver().into_option(), Some(owner));
        assert_eq!(signature.parameters(), [owner]);
        let root = input
            .materialization()
            .callable_roots()
            .iter()
            .find(|root| {
                root.subject()
                    == scoop_mir::CallableSignatureSubject::Strong(
                        binding.implementation().callable_owner(),
                    )
            })
            .unwrap();
        let function = &input.module().functions[root.function()];
        assert_eq!(function.gc_effect, binding.lowered_signature().gc_effect());
        assert_eq!(function.return_ty, scoop_mir::Type::Boolean);
        assert_eq!(function.params.len(), 2);
        assert_eq!(function.params[0].ty, function.params[1].ty);
        assert!(!function.body.blocks.is_empty());
    }
}

pub(super) fn dump(input: &ConeMirInput, bindings: &CanonicalMirCallableBindingsV1) -> String {
    let mut lines = Vec::new();
    for binding in bindings.entries() {
        let root = input
            .materialization()
            .callable_roots()
            .iter()
            .find(|root| {
                root.subject()
                    == scoop_mir::CallableSignatureSubject::Strong(
                        binding.implementation().callable_owner(),
                    )
            })
            .unwrap();
        let function = &input.module().functions[root.function()];
        let calls: Vec<_> = function
            .body
            .blocks
            .iter()
            .flat_map(|(_, block)| &block.statements)
            .filter_map(|statement| match &statement.kind {
                scoop_mir::StatementKind::Call(
                    scoop_mir::CallEffect::Unit(call) | scoop_mir::CallEffect::Value { call, .. },
                ) => match call.target.callee {
                    scoop_mir::Callee::User(target) => {
                        Some(input.module().functions[target].name.as_str())
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect();
        lines.push(format!(
            "{}: {:?} -> {:?}; calls={calls:?}\n",
            function.name,
            binding.semantic_signature().gc_effect(),
            binding.lowered_signature().gc_effect()
        ));
    }
    lines.sort();
    lines.concat()
}

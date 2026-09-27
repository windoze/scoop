use super::*;
use scoop_identity::{CallableTemplateOwner, StrongCallableDefinitionOwner};
use scoop_mir::MirCallableLoweringRoleV1 as Role;

fn root(
    input: &scoop_mir::ConeMirInput,
    binding: &scoop_mir::ParamFreeMirCallableBindingV1,
) -> scoop_mir::CallableMaterializationRoot {
    *input
        .materialization()
        .callable_roots()
        .iter()
        .find(|root| {
            root.subject()
                == scoop_mir::CallableSignatureSubject::Strong(
                    binding.implementation().callable_owner(),
                )
        })
        .unwrap()
}

pub(super) fn actual(
    output: &hir::DependencyHirOutput,
    input: &scoop_mir::ConeMirInput,
    bindings: &CanonicalMirCallableBindingsV1,
) {
    let local = output.output().local.module();
    for binding in bindings.entries() {
        let root = root(input, binding);
        let record = input
            .module()
            .meta
            .source_callable_materializations
            .get(root.function())
            .unwrap();
        assert_eq!(
            record.signature_record().signature(),
            binding.lowered_signature().exact()
        );
        assert_eq!(
            input.module().functions[root.function()].gc_effect,
            binding.lowered_signature().gc_effect()
        );
        assert_eq!(binding.semantic_signature(), binding.lowered_signature());
        let source = local
            .functions
            .iter()
            .find(|(_, function)| function.materialization == record.materialization())
            .unwrap()
            .1;
        let receiver = source
            .receiver
            .value_type()
            .map(|ty| local.exact_type_identities[ty].id());
        assert_eq!(
            binding
                .semantic_signature()
                .exact()
                .receiver()
                .into_option(),
            receiver
        );
        let parameters: Vec<_> = source
            .params
            .iter()
            .skip(usize::from(receiver.is_some()))
            .map(|parameter| local.exact_type_identities[parameter.ty].id())
            .collect();
        assert_eq!(
            binding.semantic_signature().exact().parameters(),
            parameters
        );
        assert_eq!(
            binding.semantic_signature().exact().result(),
            local.exact_type_identities[source.return_ty].id()
        );
        match (record.materialization().template(), binding.lowering_role()) {
            (
                CallableTemplateOwner::Function(id),
                Role::Ordinary | Role::PureVirtualTrap { .. },
            ) => assert_eq!(
                binding.implementation(),
                StrongCallableDefinitionOwner::Function(id)
            ),
            (
                CallableTemplateOwner::Accessor(id),
                Role::Accessor | Role::PureVirtualTrap { .. },
            ) => assert_eq!(
                binding.implementation(),
                StrongCallableDefinitionOwner::PropertyAccessor(id)
            ),
            other => panic!("unexpected source callable: {other:?}"),
        }
    }
}

pub(super) fn dump(
    input: &scoop_mir::ConeMirInput,
    bindings: &CanonicalMirCallableBindingsV1,
) -> String {
    let mut lines = Vec::new();
    for binding in bindings.entries() {
        let root = root(input, binding);
        let function = &input.module().functions[root.function()];
        let role = match binding.lowering_role() {
            Role::Ordinary => "ordinary",
            Role::Accessor => "accessor",
            Role::PureVirtualTrap { .. } => "trap",
            other => panic!("{other:?}"),
        };
        let signature = binding.semantic_signature();
        lines.push(format!(
            "{}: {role} receiver={} parameters={} {:?}\n",
            function.name,
            signature.exact().receiver().is_present(),
            signature.exact().parameters().len(),
            signature.gc_effect()
        ));
    }
    lines.sort();
    lines.concat()
}

pub(super) fn combined(
    input: &scoop_mir::ConeMirInput,
    bindings: &CanonicalMirCallableBindingsV1,
    dump: &str,
) {
    for name in [
        "Base.hidden",
        "Base.$set$slot",
        "Contract.echo",
        "Contract.get",
        "Contract.$get$token",
        "Abstract.run",
        "Again.pass",
        "Again.$get$abstractValue",
        "Registry.$get$value",
        "Registry.$set$value",
        "Registry.$get$direct",
    ] {
        assert!(
            dump.contains(&format!("{name}:")),
            "missing {name}:\n{dump}"
        );
    }
    assert!(!dump.contains("Base.secret:"));
    let secret = input
        .module()
        .functions
        .iter()
        .find(|(_, function)| function.name == "Base.secret")
        .unwrap()
        .0;
    assert!(
        input
            .materialization()
            .callable_roots()
            .iter()
            .any(|root| root.function() == secret)
    );
    assert!(
        !bindings
            .entries()
            .iter()
            .any(|binding| root(input, binding).function() == secret)
    );
}

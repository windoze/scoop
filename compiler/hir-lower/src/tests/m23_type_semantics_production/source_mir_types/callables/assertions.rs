use super::*;
use scoop_identity::{CallableTemplateOwner, StrongCallableDefinitionOwner};
use scoop_mir::MirCallableLoweringRoleV1 as Role;

pub(super) fn is_source(binding: &scoop_mir::ParamFreeMirCallableBindingV1) -> bool {
    matches!(
        binding.origin(),
        scoop_mir::MirCallableOriginV1::Function(_) | scoop_mir::MirCallableOriginV1::Accessor(_)
    )
}

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
                    binding
                        .implementation()
                        .strong_owner()
                        .unwrap()
                        .callable_owner(),
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
    for binding in bindings
        .entries()
        .iter()
        .filter(|binding| is_source(binding))
    {
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
                scoop_identity::CallableDefinitionOwner::Strong(
                    StrongCallableDefinitionOwner::Function(id)
                )
            ),
            (
                CallableTemplateOwner::Accessor(id),
                Role::Accessor | Role::PureVirtualTrap { .. },
            ) => assert_eq!(
                binding.implementation(),
                scoop_identity::CallableDefinitionOwner::Strong(
                    StrongCallableDefinitionOwner::PropertyAccessor(id)
                )
            ),
            other => panic!("unexpected source callable: {other:?}"),
        }
    }
}

pub(super) fn combined(input: &scoop_mir::ConeMirInput, bindings: &CanonicalMirCallableBindingsV1) {
    let names = bindings
        .entries()
        .iter()
        .filter(|binding| is_source(binding))
        .map(|binding| {
            input.module().functions[root(input, binding).function()]
                .name
                .as_str()
        })
        .collect::<BTreeSet<_>>();
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
        assert!(names.contains(name), "missing {name}");
    }
    assert!(!names.contains("Base.secret"));
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
            .filter(|binding| is_source(binding))
            .any(|binding| root(input, binding).function() == secret)
    );
}

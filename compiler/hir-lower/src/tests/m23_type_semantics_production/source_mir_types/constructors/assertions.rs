use super::*;
use scoop_identity::{
    CallableMaterializationContext, CallableTemplateOwner, StrongCallableDefinitionOwner,
};
use scoop_mir::MirCallableLoweringRoleV1;

pub(super) fn actual(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
    input: &scoop_mir::ConeMirInput,
    bindings: &CanonicalMirCallableBindingsV1,
) {
    let constructors = hir::select_param_free_source_constructors(
        output.output().export.cone,
        public,
        &source_inventory::identity_closure(output),
    )
    .unwrap();
    assert_eq!(bindings.entries().len(), constructors.len());
    let local = output.output().local.module();
    if let Some(private) = source_dispatch::owners(output).get("Private") {
        let root = input
            .module()
            .meta
            .source_callable_materializations
            .iter()
            .find(|record| {
                record
                    .signature_record()
                    .signature()
                    .receiver()
                    .into_option()
                    == Some(*private)
            })
            .expect("the private constructor is actually materialized");
        let CallableTemplateOwner::Constructor(id) = root.materialization().template() else {
            panic!("the private initializer retains its constructor identity")
        };
        assert!(
            bindings
                .get(StrongCallableDefinitionOwner::Constructor(id))
                .is_none()
        );
    }
    for binding in bindings.entries() {
        let scoop_identity::CallableDefinitionOwner::Strong(
            StrongCallableDefinitionOwner::Constructor(declaration),
        ) = binding.implementation()
        else {
            panic!("only constructors are produced")
        };
        let actual: Vec<_> = input
            .module()
            .meta
            .source_callable_materializations
            .iter()
            .filter(|record| {
                record.materialization().template()
                    == CallableTemplateOwner::Constructor(declaration)
            })
            .collect();
        assert_eq!(actual.len(), 1);
        assert_eq!(
            actual[0].materialization().context(),
            CallableMaterializationContext::NoSubstitution
        );
        assert_eq!(
            actual[0].signature_record().signature(),
            binding.lowered_signature().exact()
        );
        assert_eq!(
            input.module().functions[actual[0].function()].gc_effect,
            binding.lowered_signature().gc_effect()
        );
        let semantic = binding.semantic_signature().exact();
        let lowered = binding.lowered_signature().exact();
        assert!(!semantic.receiver().is_present());
        match *binding.lowering_role() {
            MirCallableLoweringRoleV1::ClassInitializer { owner } => {
                assert_eq!(semantic.result(), owner);
                assert_eq!(lowered.receiver().into_option(), Some(owner));
                assert_eq!(
                    lowered.result(),
                    input
                        .module()
                        .meta
                        .source_exact_types
                        .get(&scoop_mir::Type::Unit)
                        .unwrap()
                        .identity_record()
                        .id()
                );
                let source = local
                    .class_constructors
                    .iter()
                    .find_map(|(_, record)| {
                        (record.materialization.template()
                            == CallableTemplateOwner::Constructor(declaration))
                        .then_some(record)
                    })
                    .unwrap();
                let parameters: Vec<_> = source
                    .parameters
                    .iter()
                    .map(|parameter| local.exact_type_identities[parameter.ty].id())
                    .collect();
                assert_eq!(semantic.parameters(), parameters);
            }
            MirCallableLoweringRoleV1::ValueConstructor { owner }
            | MirCallableLoweringRoleV1::PrimaryValueConstructor { owner } => {
                assert_eq!(semantic.result(), owner);
                assert_eq!(semantic, lowered);
                let source = local
                    .struct_constructors
                    .iter()
                    .find_map(|(_, record)| {
                        (record.materialization.template()
                            == CallableTemplateOwner::Constructor(declaration))
                        .then_some(record)
                    })
                    .unwrap();
                let parameters: Vec<_> = source
                    .parameters
                    .iter()
                    .map(|parameter| local.exact_type_identities[parameter.ty].id())
                    .collect();
                assert_eq!(semantic.parameters(), parameters);
            }
            other => panic!("unexpected constructor role: {other:?}"),
        }
    }
    let private: Vec<_> = input
        .module()
        .meta
        .source_callable_materializations
        .iter()
        .filter_map(|record| match record.materialization().template() {
            CallableTemplateOwner::Constructor(id) if !constructors.contains_key(&id) => Some(id),
            _ => None,
        })
        .collect();
    for id in private {
        assert!(
            bindings
                .get(StrongCallableDefinitionOwner::Constructor(id))
                .is_none()
        );
    }
}

pub(super) fn dump(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
    bindings: &CanonicalMirCallableBindingsV1,
) -> String {
    let owners = source_dispatch::owners(output);
    let name = |id| {
        owners
            .iter()
            .find(|(_, exact)| **exact == id)
            .unwrap()
            .0
            .as_str()
    };
    let mut lines = Vec::new();
    for binding in bindings.entries() {
        let scoop_identity::CallableDefinitionOwner::Strong(
            StrongCallableDefinitionOwner::Constructor(id),
        ) = binding.implementation()
        else {
            unreachable!()
        };
        let semantic = binding.semantic_signature();
        let owner = name(semantic.exact().result());
        let parameters = semantic
            .exact()
            .parameters()
            .iter()
            .map(|id| name(*id))
            .collect::<Vec<_>>()
            .join(", ");
        let role = match binding.lowering_role() {
            MirCallableLoweringRoleV1::ClassInitializer { .. } => {
                format!("receiver={owner} result=Unit")
            }
            MirCallableLoweringRoleV1::ValueConstructor { .. }
            | MirCallableLoweringRoleV1::PrimaryValueConstructor { .. } => {
                format!("receiver=none result={owner}")
            }
            _ => unreachable!(),
        };
        let access = public
            .callable_interfaces()
            .declaration(scoop_identity::CallableTemplateOrigin::Constructor(id))
            .unwrap()
            .declared_visibility();
        lines.push(format!(
            "{owner}({parameters}): {access:?} {:?} => {role} {:?}\n",
            semantic.gc_effect(),
            binding.lowered_signature().gc_effect()
        ));
    }
    lines.sort();
    lines.concat()
}

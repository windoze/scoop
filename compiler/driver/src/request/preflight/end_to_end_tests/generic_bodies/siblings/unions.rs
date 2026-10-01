use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    ConeIdentity, DeclarationName, ExactCallableSignature, ExactTypeKey, GeneratedCallableKey,
    OdrMemberDiscriminator, OdrMemberRole, SourceDeclarationKey, SpecializationKey,
    StrongCallableDefinitionOwner,
};
use scoop_slib::LinkSymbolsReplayedCrossConeLayoutClosure;

pub(super) fn check_members(
    closure: &LinkSymbolsReplayedCrossConeLayoutClosure,
    left: ConeIdentity,
    right: ConeIdentity,
) {
    let mut targets = BTreeMap::<ExactCallableSignature, Vec<_>>::new();
    for definition in closure.odr_definitions().members() {
        let OdrMemberDiscriminator::GeneratedCallable(callable) = definition.key().discriminator()
        else {
            continue;
        };
        let providers = definition
            .candidates()
            .map(|candidate| candidate.provider())
            .collect::<BTreeSet<_>>();
        if !providers.contains(&left) && !providers.contains(&right) {
            continue;
        }
        let candidate = definition.candidates().next().unwrap();
        let (sections, _) = closure.artifact(candidate.provider()).unwrap();
        let key = sections
            .identity_graph()
            .canonical_key::<_, GeneratedCallableKey>(*callable)
            .unwrap();
        if let GeneratedCallableKey::FunctionAdapter { source, target } = key.as_ref() {
            targets.entry(target.clone()).or_default().push((
                definition,
                source.clone(),
                providers,
            ));
        }
    }
    let (target, adapters) = targets
        .iter()
        .find(|(_, adapters)| {
            adapters
                .iter()
                .any(|(_, _, providers)| providers.contains(&left) && !providers.contains(&right))
                && adapters.iter().any(|(_, _, providers)| {
                    providers.contains(&right) && !providers.contains(&left)
                })
        })
        .expect("siblings contribute independent adapters to one target signature");
    let group = adapters[0].0.key().group();
    let exact = scoop_identity::PersistentExactTypeId::from_key(&ExactTypeKey::Function {
        effect: target.effect(),
        parameters: target.parameters().to_vec(),
        result: target.result(),
    })
    .unwrap();
    let sources = adapters
        .iter()
        .map(|(_, source, _)| source)
        .collect::<BTreeSet<_>>();
    assert!(sources.len() >= 2, "the source signatures differ");
    for (definition, _, _) in adapters {
        assert_eq!(definition.key().group(), group);
        assert_eq!(
            definition.group_key(),
            &SpecializationKey::StructuralType { exact_type: exact }
        );
    }
    let descriptor = closure
        .odr_definitions()
        .members()
        .find(|member| {
            member.key().group() == group
                && member.key().role() == OdrMemberRole::TypeDescriptor
                && member.key().discriminator() == &OdrMemberDiscriminator::ExactType(exact)
        })
        .expect("the shared target function shape has its actual descriptor");
    let providers = descriptor
        .candidates()
        .map(|candidate| candidate.provider())
        .collect::<BTreeSet<_>>();
    assert!(providers.contains(&left) && providers.contains(&right));
}

pub(super) fn runtime_source(
    closure: &LinkSymbolsReplayedCrossConeLayoutClosure,
    provider: ConeIdentity,
    left: ConeIdentity,
    right: ConeIdentity,
    base: &str,
    probes: &str,
) -> String {
    let (sections, _) = closure.artifact(provider).unwrap();
    let interface = sections
        .mir_type_bridge()
        .exports()
        .types()
        .records()
        .iter()
        .find(|ty| {
            let scoop_mir::MirTypeOriginV1::SourceNominal(owner) = ty.origin() else {
                return false;
            };
            let key = sections
                .identity_graph()
                .canonical_key::<_, SourceDeclarationKey>(*owner)
                .unwrap();
            matches!(key.name(), DeclarationName::Named(name) if name.as_str() == "Readable")
        })
        .unwrap();
    let descriptor = sections
        .lir_exports()
        .descriptors()
        .get(interface.exact())
        .unwrap();
    let slots = sections
        .mir_type_bridge()
        .exports()
        .dispatch()
        .get(interface.exact())
        .unwrap()
        .interface_slots()
        .unwrap();
    assert_eq!(slots.len(), 1);
    let probes = probes
        .replace("UNION_LEFT_OBJECT", &symbol(closure, left, "sameType"))
        .replace("UNION_RIGHT_OBJECT", &symbol(closure, right, "sameType"))
        .replace("UNION_OTHER_OBJECT", &symbol(closure, right, "otherType"))
        .replace(
            "UNION_INTERFACE_TD",
            &format!("_{}", descriptor.definition().symbol().symbol()),
        )
        .replace("UNION_READ_SLOT", &slots[0].position().get().to_string());
    format!("{probes}\n{base}")
}

fn symbol(
    closure: &LinkSymbolsReplayedCrossConeLayoutClosure,
    provider: ConeIdentity,
    name: &str,
) -> String {
    let (sections, _) = closure.artifact(provider).unwrap();
    let symbols = sections
        .lir_cross_cone_bridge()
        .exports()
        .iter()
        .filter_map(|export| {
            let StrongCallableDefinitionOwner::Function(callable) = export.target() else {
                return None;
            };
            let key = sections
                .identity_graph()
                .canonical_key::<_, SourceDeclarationKey>(callable)
                .unwrap();
            matches!(key.name(), DeclarationName::Named(actual) if actual.as_str() == name)
                .then(|| format!("_{}", export.expected_symbol().symbol()))
        })
        .collect::<Vec<_>>();
    assert_eq!(symbols.len(), 1, "one actual {name} probe");
    symbols.into_iter().next().unwrap()
}

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    ConeIdentity, DeclarationName, ExactCallableSignature, ExactTypeKey, GeneratedCallableKey,
    OdrMemberDiscriminator, OdrMemberRole, SourceDeclarationKey, SpecializationKey,
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
        .replace(
            "UNION_LEFT_TD",
            &symbol(closure, left, scoop_mir::IntegerKind::SIGNED_32),
        )
        .replace(
            "UNION_RIGHT_TD",
            &symbol(closure, right, scoop_mir::IntegerKind::SIGNED_32),
        )
        .replace(
            "UNION_OTHER_TD",
            &symbol(closure, right, scoop_mir::IntegerKind::SIGNED_64),
        )
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
    integer: scoop_mir::IntegerKind,
) -> String {
    let argument = closure.dependency_first().find_map(|(sections, _)| {
        sections.mir_type_bridge().exports().types().records().iter().find_map(|record| {
            matches!(record.representation(), scoop_mir::MirTypeRepresentationV1::Intrinsic(scoop_mir::MirParamFreeIntrinsicV1::Integer(kind)) if *kind == integer).then_some(record.exact())
        })
    }).unwrap();
    let (sections, _) = closure.artifact(provider).unwrap();
    let symbols = sections
        .lir_exports()
        .descriptors()
        .records()
        .iter()
        .filter_map(|record| {
            let key = sections
                .identity_graph()
                .canonical_key::<_, ExactTypeKey>(record.exact())
                .unwrap();
            let ExactTypeKey::NominalApplication { origin, arguments } = key.as_ref() else {
                return None;
            };
            if arguments.as_slice() != [argument] {
                return None;
            }
            let origin = sections
                .identity_graph()
                .canonical_key::<_, SourceDeclarationKey>(*origin)
                .unwrap();
            matches!(origin.name(), DeclarationName::Named(name) if name.as_str() == "Marker")
                .then(|| format!("_{}", record.definition().symbol().symbol()))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        symbols.len(),
        1,
        "one actual Marker descriptor for {integer:?}"
    );
    symbols.into_iter().next().unwrap()
}

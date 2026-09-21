use super::*;

fn shape(ty: &SignatureTypeKey) -> String {
    match ty {
        SignatureTypeKey::Binder { depth, index } => format!("binder({depth},{index})"),
        SignatureTypeKey::Nominal(_) => "nominal".into(),
        SignatureTypeKey::NominalApplication { arguments, .. } => format!(
            "application<{}>",
            arguments
                .as_slice()
                .iter()
                .map(shape)
                .collect::<Vec<_>>()
                .join(",")
        ),
        other => panic!("unexpected fixture type {other:?}"),
    }
}

pub(super) fn verify(output: &hir::OrdinaryHirOutput, source: &Table) -> String {
    let export = output.output().export.module();
    let ids = source
        .records()
        .iter()
        .map(|r| CallableTemplateOrigin::Constructor(r.declaration()))
        .collect();
    let protocols = hir::CanonicalNominalSourceParameterProtocolsV1::from_export_hir(
        &output.output().export,
        &ids,
        &mut meter(),
    )
    .unwrap();
    let mut origins = BTreeMap::new();
    for (id, constructor) in export.class_constructors.iter() {
        if let Some(identity) = export.constructor_identities[id].source_record() {
            origins.insert(
                identity.id(),
                (
                    export.classes[constructor.owner].name.as_str(),
                    identity.key(),
                    constructor.safety,
                    hir::GcEffect::Managed,
                ),
            );
        }
    }
    for (id, constructor) in export.struct_constructors.iter() {
        let identity = &export.constructor_identities[id];
        origins.insert(
            identity.id(),
            (
                export.structs[constructor.owner].name.as_str(),
                identity.key(),
                constructor.safety,
                constructor.source_gc_effect(),
            ),
        );
    }
    let mut rows = Vec::new();
    for record in source.records() {
        let (name, key, safety, gc_effect) = origins[&record.declaration()];
        let payload = record.payload();
        assert_eq!(
            payload.effects().safety(),
            match safety {
                hir::Safety::Safe => hir::CallableSafetyV1::Safe,
                hir::Safety::Unsafe => hir::CallableSafetyV1::Unsafe,
            }
        );
        assert_eq!(
            payload.effects().gc_effect(),
            match gc_effect {
                hir::GcEffect::Managed => scoop_identity::GcEffect::Managed,
                hir::GcEffect::NoGc => scoop_identity::GcEffect::NoGc,
            }
        );
        assert_eq!(
            payload.effects().execution(),
            scoop_identity::Effect::Ordinary
        );
        assert_eq!(payload.modality(), hir::CallableModalityV1::Final);
        assert!(payload.type_parameters().is_empty());
        assert!(!payload.receiver().is_present());
        assert!(payload.slot_relations().is_empty());
        assert_eq!(
            record.declaration_access().lexical_owners().last(),
            Some(&payload.owner())
        );
        let scoop_identity::DuplicateSignatureKey::Constructor { parameters } =
            key.duplicate_signature()
        else {
            panic!("constructor key")
        };
        assert_eq!(
            payload
                .parameters()
                .parameters()
                .iter()
                .map(|p| p.value_type())
                .collect::<Vec<_>>(),
            parameters.iter().collect::<Vec<_>>()
        );
        let protocol = protocols
            .get(CallableTemplateOrigin::Constructor(record.declaration()))
            .unwrap();
        assert_eq!(
            payload.parameters().parameters(),
            protocol
                .parameters()
                .iter()
                .map(|p| p.shape().clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            record.declaration_access().definition_origin().origin(),
            export
                .export_definition_origins
                .get(DefinitionOriginSubject::Constructor(record.declaration()))
                .unwrap()
                .origin()
        );
        assert_eq!(source.get(record.declaration()), Some(record));
        match (payload.owner(), payload.result()) {
            (hir::SourceNominalId::Concrete(owner), SignatureTypeKey::Nominal(result)) => {
                assert_eq!(owner, *result)
            }
            (
                hir::SourceNominalId::GenericTemplate(owner),
                SignatureTypeKey::NominalApplication { origin, arguments },
            ) => {
                assert_eq!(owner, *origin);
                for (index, argument) in arguments.as_slice().iter().enumerate() {
                    assert_eq!(
                        *argument,
                        SignatureTypeKey::Binder {
                            depth: 0,
                            index: index as u32
                        }
                    );
                }
            }
            other => panic!("constructor self result: {other:?}"),
        }
        let parameters = payload
            .parameters()
            .parameters()
            .iter()
            .map(|p| format!("{}:{}", p.name().as_str(), shape(p.value_type())))
            .collect::<Vec<_>>()
            .join(",");
        rows.push(format!(
            "{name}({parameters})->{}: {:?} {:?} {:?}, owners={}\n",
            shape(payload.result()),
            record.declaration_access().declared_visibility(),
            payload.effects().safety(),
            payload.effects().gc_effect(),
            record.declaration_access().lexical_owners().len()
        ));
    }
    rows.sort();
    rows.concat()
}

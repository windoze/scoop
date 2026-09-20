use super::*;
use scoop_identity::{CanonicalIdentifier, EnumVariantFieldSelector, SignatureTypeKey};

fn candidate(
    bound: &hir::BoundInheritanceSourcesV1<'_, '_, '_>,
    source: &hir::NominalSourceContractV1,
    variant: &hir::EnumSourceVariantV1,
) -> hir::NominalSupportCallableInterfaceV1 {
    let owner = source.owner();
    let declaration = CallableTemplateOrigin::VariantConstructor(variant.variant());
    let result = match owner {
        hir::SourceNominalId::Concrete(id) => SignatureTypeKey::Nominal(id),
        hir::SourceNominalId::GenericTemplate(origin) => SignatureTypeKey::NominalApplication {
            origin,
            arguments: scoop_identity::NonEmptyVec::new(
                source
                    .type_parameters()
                    .binders()
                    .iter()
                    .enumerate()
                    .map(|(index, _)| SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    })
                    .collect(),
            )
            .unwrap(),
        },
    };
    let parameters = variant
        .fields()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let name = match bound
                .source_enum_variant_field_key(field.field())
                .unwrap()
                .selector()
            {
                EnumVariantFieldSelector::Named(name) => name.clone(),
                EnumVariantFieldSelector::Positional { .. } => {
                    CanonicalIdentifier::new(&format!("p{index}")).unwrap()
                }
            };
            hir::SourceParameterShapeV1::new(name, field.value_type().clone())
        })
        .collect();
    let effects = hir::CallableSourceEffectsV1::try_new(
        scoop_identity::Effect::Ordinary,
        hir::CallableSafetyV1::Safe,
        scoop_identity::GcEffect::Managed,
        hir::CallableImplementationV1::Scoop,
        hir::CallableOperatorRoleV1::None,
        hir::CallableInfixV1::Ordinary,
    )
    .unwrap();
    let payload = hir::NominalSourceCallablePayloadV1::try_new(
        declaration,
        owner,
        hir::CanonicalBinderListV1::try_new(vec![]).unwrap(),
        hir::CanonicalSourceParameterShapesV1::try_new(parameters).unwrap(),
        result,
        effects,
        hir::CallableModalityV1::Final,
        hir::CanonicalProtectedSlotRefsV1::try_new(vec![]).unwrap(),
    )
    .unwrap();
    let mut owners = hir::NominalInheritanceSemanticAuthority::nominal_access_source(bound, owner)
        .unwrap()
        .lexical_owners()
        .to_vec();
    owners.push(owner);
    let access = hir::DeclarationAccessSourceV1::try_new(
        hir::DeclaredVisibilityV1::Public,
        owners,
        bound
            .source_enum_variant_origin(variant.variant())
            .unwrap()
            .clone(),
    )
    .unwrap();
    hir::NominalSupportCallableInterfaceV1::try_new(declaration, access, payload).unwrap()
}

#[test]
fn inheritance_sources_replay_real_generic_and_concrete_enum_constructors() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core
            .foundation
            .import_core_inputs(&core.interface, &[])
            .unwrap();
        sources
            .with_bound(
                &foundation,
                inputs.protocols().fundamental_types(),
                &mut meter(),
                |bound, graph| {
                    let mut count = 0;
                    for source in sources.nominals.records() {
                        let hir::NominalSourceShapeV1::Enum(shape) = source.source_shape() else {
                            continue;
                        };
                        for variant in shape.variants() {
                            let record = candidate(bound, source, variant);
                            record.validate_source(graph, bound, &mut meter()).unwrap();
                            count += 1;
                            let other = shape
                                .variants()
                                .iter()
                                .find(|other| other.variant() != variant.variant())
                                .unwrap();
                            let access = hir::DeclarationAccessSourceV1::try_new(
                                hir::DeclaredVisibilityV1::Public,
                                record.declaration_access().lexical_owners().to_vec(),
                                bound
                                    .source_enum_variant_origin(other.variant())
                                    .unwrap()
                                    .clone(),
                            )
                            .unwrap();
                            let wrong_origin = hir::NominalSupportCallableInterfaceV1::try_new(
                                record.declaration(),
                                access,
                                record.payload().clone(),
                            )
                            .unwrap();
                            assert!(matches!(
                                wrong_origin.validate_source(graph, bound, &mut meter()),
                                Err(hir::NominalSupportCallableSemanticError::Variant(
                                    hir::NominalSupportVariantError::Access
                                ))
                            ));
                        }
                    }
                    assert_eq!(count, 8);
                },
            )
            .unwrap();
    });
}

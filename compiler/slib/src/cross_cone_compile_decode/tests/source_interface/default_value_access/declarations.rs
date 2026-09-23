use super::*;

pub(in super::super) fn restrict(
    front: &mut HirProductionValidatedCrossConeHirFrontSections<'_>,
    subject: DefinitionOriginSubject,
    visibility: DeclaredVisibilityV1,
) {
    let interface = &front.hir_interface;
    let mut nominals = interface.nominal_interfaces().clone();
    let mut callables = interface.callable_interfaces().clone();
    let mut properties = interface.property_interfaces().clone();
    match subject {
        DefinitionOriginSubject::Type(id) => {
            let declaration = SourceNominalId::Concrete(id);
            let record = nominals.declaration(declaration).unwrap();
            let details = record.declaration_details();
            let restricted = NominalInterfaceRecordV1::try_new(
                declaration,
                record.kind(),
                record.type_parameters().clone(),
                record.exact_supertypes().clone(),
                CanonicalPersistentIdsV1::empty(),
                CanonicalPublicMemberRefsV1::default(),
                CanonicalPersistentIdsV1::empty(),
                record.source_shape().clone(),
                NominalDeclarationDetailsV1::new(
                    details.modality(),
                    visibility,
                    details.constructors().clone(),
                    details.members().clone(),
                    details.children().clone(),
                ),
            )
            .unwrap();
            nominals = CanonicalNominalInterfacesV1::with_support(
                nominals
                    .records()
                    .iter()
                    .filter(|r| r.declaration() != declaration)
                    .cloned()
                    .collect(),
                vec![restricted],
            )
            .unwrap();
        }
        DefinitionOriginSubject::Constructor(_)
        | DefinitionOriginSubject::Function(_)
        | DefinitionOriginSubject::GenericFunction(_)
        | DefinitionOriginSubject::PropertyAccessor(_) => {
            let declaration = match subject {
                DefinitionOriginSubject::Constructor(id) => CallableTemplateOrigin::Constructor(id),
                DefinitionOriginSubject::Function(id) => CallableTemplateOrigin::Function(id),
                DefinitionOriginSubject::GenericFunction(id) => {
                    CallableTemplateOrigin::GenericFunction(id)
                }
                DefinitionOriginSubject::PropertyAccessor(id) => {
                    CallableTemplateOrigin::Accessor(id)
                }
                _ => panic!("callable subject"),
            };
            let record = callables.declaration(declaration).unwrap();
            let restricted = CallableDeclarationRecordV1::try_new(
                declaration,
                record.owner(),
                record.type_parameters().clone(),
                record.receiver().cloned(),
                record.parameters().clone(),
                record.result().clone(),
                record.effects(),
                record.modality(),
                visibility,
                record.slot_relations().clone(),
            )
            .unwrap();
            let mut support = callables
                .support_records()
                .iter()
                .filter(|r| r.declaration() != declaration)
                .cloned()
                .collect::<Vec<_>>();
            support.push(restricted);
            callables = CanonicalCallableInterfacesV1::with_support(
                callables
                    .records()
                    .iter()
                    .filter(|r| r.declaration() != declaration)
                    .cloned()
                    .collect(),
                support,
            )
            .unwrap();
        }
        DefinitionOriginSubject::Property(id) => {
            let declaration = scoop_identity::PropertyOwner::Property(id);
            let record = properties.declaration(declaration).unwrap();
            let restricted = PropertyDeclarationRecordV1::try_new(
                declaration,
                record.owner(),
                record.type_parameters().clone(),
                record.receiver().cloned(),
                record.value_type().clone(),
                record.accessors(),
                record.representation(),
                visibility,
            )
            .unwrap();
            properties = CanonicalPropertyInterfacesV1::with_support(
                properties
                    .records()
                    .iter()
                    .filter(|r| r.declaration() != declaration)
                    .cloned()
                    .collect(),
                vec![restricted],
            )
            .unwrap();
        }
        _ => panic!("fixture only restricts value declarations"),
    }
    front.hir_interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        nominals,
        callables,
        properties,
        interface.type_aliases().clone(),
        interface.source_interfaces().clone(),
        interface.default_templates().clone(),
        interface.constants().clone(),
        interface.definition_sources().clone(),
        interface.external_references().clone(),
    );
}

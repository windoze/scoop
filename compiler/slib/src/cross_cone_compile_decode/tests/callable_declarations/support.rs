use super::*;

pub(super) struct Fixture {
    foundation: CanonicalHirFoundation,
    pub interface: CrossConeHirInterfaceSectionV1,
    pub function: CallableTemplateOrigin,
    pub constructor: CallableTemplateOrigin,
}
impl Fixture {
    pub fn new() -> Self {
        let (foundation, wire, owner, function, constructor, ..) =
            nominal_surface(cone().identity(), true, true, true);
        let original =
            cross_cone_artifact_for_with_hir_foundation(cone(), vec![], &foundation, wire);
        let checked = nominal_fields::front(&original)
            .validate_nominal_surface(vec![])
            .unwrap();
        let section = checked.hir_interface();
        let function = CallableTemplateOrigin::Function(function);
        let constructor = CallableTemplateOrigin::Constructor(constructor);
        let public = section
            .callable_interfaces()
            .records()
            .iter()
            .filter(|r| r.declaration() != function && r.declaration() != constructor)
            .cloned()
            .collect();
        let mut support = [function, constructor]
            .map(|id| {
                let record = section.callable_interfaces().get(id).unwrap();
                declaration(record.declaration_data(), record.result().clone())
            })
            .to_vec();
        support.extend_from_slice(section.callable_interfaces().support_records());
        let record = section
            .nominal_interfaces()
            .get(SourceNominalId::Concrete(owner))
            .unwrap();
        let nominal = NominalInterfaceRecordV1::try_new(
            record.declaration(),
            record.kind(),
            record.type_parameters().clone(),
            record.exact_supertypes().clone(),
            CanonicalPersistentIdsV1::empty(),
            CanonicalPublicMemberRefsV1::default(),
            record.nested_bindings().clone(),
            record.source_shape().clone(),
            record.declaration_details().clone(),
        )
        .unwrap();
        let interface = CrossConeHirInterfaceSectionV1::new(
            section.public_bindings().clone(),
            CanonicalNominalInterfacesV1::try_new(vec![nominal]).unwrap(),
            CanonicalCallableInterfacesV1::with_support(public, support).unwrap(),
            section.property_interfaces().clone(),
            section.type_aliases().clone(),
            section.source_interfaces().clone(),
            section.default_templates().clone(),
            section.constants().clone(),
            section.definition_sources().clone(),
            section.external_references().clone(),
            section.generic_callable_bodies().clone(),
            section.generic_initializations().clone(),
        );
        Self {
            foundation,
            interface,
            function,
            constructor,
        }
    }

    pub fn artifact(&self, interface: &CrossConeHirInterfaceSectionV1) -> Vec<u8> {
        let mut interface = interface.clone();
        cross_cone_artifact_for_with_hir_foundation(
            cone(),
            vec![],
            &self.foundation,
            encode(&interface.index_for_wire().unwrap()).unwrap(),
        )
    }

    pub fn with_callables(
        &self,
        callables: CanonicalCallableInterfacesV1,
    ) -> CrossConeHirInterfaceSectionV1 {
        let section = &self.interface;
        CrossConeHirInterfaceSectionV1::new(
            section.public_bindings().clone(),
            section.nominal_interfaces().clone(),
            callables,
            section.property_interfaces().clone(),
            section.type_aliases().clone(),
            section.source_interfaces().clone(),
            section.default_templates().clone(),
            section.constants().clone(),
            section.definition_sources().clone(),
            section.external_references().clone(),
            section.generic_callable_bodies().clone(),
            section.generic_initializations().clone(),
        )
    }
}

pub(super) fn declaration(
    record: &CallableDeclarationRecordV1,
    result: SignatureTypeKey,
) -> CallableDeclarationRecordV1 {
    CallableDeclarationRecordV1::try_new(
        record.declaration(),
        record.owner(),
        record.type_parameters().clone(),
        record.receiver().cloned(),
        record.parameters().clone(),
        result,
        record.effects(),
        record.modality(),
        DeclaredVisibilityV1::Private,
        record.slot_relations().clone(),
    )
    .unwrap()
}

pub(super) fn front(bytes: &[u8]) -> crate::HirProductionValidatedCrossConeHirFrontSections<'_> {
    declaration_front(bytes)
}

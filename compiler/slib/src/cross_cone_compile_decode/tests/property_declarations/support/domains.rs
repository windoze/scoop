use super::*;

impl Fixture {
    pub fn set_visibilities(
        &mut self,
        property_visibility: DeclaredVisibilityV1,
        setter_visibility: DeclaredVisibilityV1,
    ) {
        let property = self.properties.records()[0].declaration_data();
        let getter = property.accessors().getter();
        let id = property.declaration();
        let property = PropertyDeclarationRecordV1::try_new(
            id,
            property.owner(),
            property.type_parameters().clone(),
            property.receiver().cloned(),
            property.value_type().clone(),
            property.accessors(),
            property.representation(),
            property_visibility,
        )
        .unwrap();
        let mut properties = self.properties.support_records().to_vec();
        let public_property = property_visibility == DeclaredVisibilityV1::Public;
        let public_setter = public_property && setter_visibility == DeclaredVisibilityV1::Public;
        let public = if public_property {
            vec![
                PropertyInterfaceRecordV1::from_declaration(
                    property,
                    PropertyPublicAccessV1::DirectOnly,
                    if public_setter {
                        PropertySetterPublicAccessV1::Public
                    } else {
                        PropertySetterPublicAccessV1::Restricted
                    },
                )
                .unwrap(),
            ]
        } else {
            properties.push(property);
            vec![]
        };
        self.properties = CanonicalPropertyInterfacesV1::with_support(public, properties).unwrap();
        let mut public = Vec::new();
        let mut support = Vec::new();
        for record in self.callables.all_declarations() {
            let is_getter = record.declaration() == CallableTemplateOrigin::Accessor(getter);
            let is_setter = record.declaration() == CallableTemplateOrigin::Accessor(self.setter);
            let visibility = if is_getter {
                property_visibility
            } else if is_setter {
                setter_visibility
            } else {
                record.declared_visibility()
            };
            let record = CallableDeclarationRecordV1::try_new(
                record.declaration(),
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
            if (is_getter && public_property) || (is_setter && public_setter) {
                public.push(
                    CallableInterfaceRecordV1::from_declaration(
                        record,
                        PublicLookupAccessV1::DirectOnly,
                    )
                    .unwrap(),
                );
            } else {
                support.push(record);
            }
        }
        self.callables = CanonicalCallableInterfacesV1::with_support(public, support).unwrap();
        self.nominal = NominalInterfaceRecordV1::try_new(
            self.nominal.declaration(),
            self.nominal.kind(),
            self.nominal.type_parameters().clone(),
            self.nominal.exact_supertypes().clone(),
            self.nominal.constructors().clone(),
            CanonicalPublicMemberRefsV1::try_new(if public_property {
                vec![PublicMemberRefV1::Property(id)]
            } else {
                vec![]
            })
            .unwrap(),
            self.nominal.nested_bindings().clone(),
            self.nominal.source_shape().clone(),
            self.nominal.declaration_details().clone(),
        )
        .unwrap();
    }

    pub fn cycle_owner_inheritance(&mut self) {
        let SourceNominalId::Concrete(owner) = self.nominal.declaration() else {
            panic!("concrete fixture owner")
        };
        self.nominal = NominalInterfaceRecordV1::try_new(
            self.nominal.declaration(),
            self.nominal.kind(),
            self.nominal.type_parameters().clone(),
            CanonicalSignatureTypesV1::try_new(vec![SignatureTypeKey::Nominal(owner)]).unwrap(),
            self.nominal.constructors().clone(),
            self.nominal.members().clone(),
            self.nominal.nested_bindings().clone(),
            self.nominal.source_shape().clone(),
            self.nominal.declaration_details().clone(),
        )
        .unwrap();
    }
}

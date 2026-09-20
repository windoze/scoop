use super::*;
use scoop_identity::{CanonicalIdentifier, CoreBuiltinNominal, SignatureTypeKey};

impl Projection<'_, '_> {
    pub(super) fn accessors(&mut self) -> Result<(), Error> {
        for (id, property) in self.export.properties.iter() {
            work(self.meter, 1)?;
            let getter = property.capability.getter();
            let declaration = CallableTemplateOrigin::Accessor(
                self.export.property_accessor_identities[getter].id(),
            );
            if self.take(declaration)? {
                let getter = &self.export.property_getters[getter];
                self.accessor(
                    id,
                    declaration,
                    getter.access.declared,
                    getter.attributes,
                    getter.implementation,
                    None,
                )?;
            }
            if let Some(setter) = property.capability.setter() {
                let declaration = CallableTemplateOrigin::Accessor(
                    self.export.property_accessor_identities[setter].id(),
                );
                if self.take(declaration)? {
                    let setter = &self.export.property_setters[setter];
                    self.accessor(
                        id,
                        declaration,
                        setter.access.declared,
                        setter.attributes,
                        setter.implementation,
                        Some(&setter.parameter_name),
                    )?;
                }
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn accessor(
        &mut self,
        property: PropertyId,
        declaration: CallableTemplateOrigin,
        visibility: DeclaredVisibility,
        attributes: FunctionAttributes,
        implementation: PropertyAccessorImplementation,
        setter_name: Option<&str>,
    ) -> Result<(), Error> {
        if visibility != DeclaredVisibility::Protected {
            return Err(invalid(
                "protected accessor inventory refers to another visibility",
            ));
        }
        let property_value = &self.export.properties[property];
        let HirPropertyIdentity::Ordinary(identity) = &self.export.property_identities[property]
        else {
            return Err(invalid(
                "protected accessor belongs to an extension property",
            ));
        };
        let CallableTemplateOrigin::Accessor(accessor) = declaration else {
            return Err(invalid("accessor has another source declaration role"));
        };
        let access = self.access(
            identity.key(),
            DefinitionOriginSubject::PropertyAccessor(accessor),
        )?;
        resources::ty(self.export, property_value.ty, 0, 3, self.meter)?;
        let value_type = self
            .signatures
            .map_type(property_value.ty, &[])
            .map_err(invalid)?;
        let (parameters, result) = if let Some(name) = setter_name {
            resources::name(name, self.meter)?;
            self.meter
                .charge_collection_slots(1, &WirePath::root())
                .map_err(resource)?;
            (
                vec![SourceParameterShapeV1::new(
                    CanonicalIdentifier::new(name).map_err(invalid)?,
                    value_type,
                )],
                SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
            )
        } else {
            (Vec::new(), value_type)
        };
        let slots = match implementation {
            PropertyAccessorImplementation::Body(function)
            | PropertyAccessorImplementation::AbstractSlot(function) => {
                let method = self.export.functions[function]
                    .method
                    .ok_or_else(|| invalid("protected accessor body has no member owner"))?;
                self.slots(method)?
            }
            PropertyAccessorImplementation::Storage => {
                CanonicalProtectedSlotRefsV1::try_new(Vec::new()).map_err(invalid)?
            }
            PropertyAccessorImplementation::Constant => {
                return Err(invalid("const property cannot export an accessor source"));
            }
        };
        let payload = ProtectedCallablePayloadV1::try_new(
            declaration,
            owner(&access)?,
            CanonicalBinderListV1::try_new(Vec::new()).map_err(invalid)?,
            CanonicalSourceParameterShapesV1::try_new(parameters).map_err(invalid)?,
            result,
            callable_interfaces::source_accessor_effects(attributes).map_err(invalid)?,
            modality(property_value.modifier),
            slots,
        )
        .map_err(invalid)?;
        self.push(declaration, access, payload)
    }
}

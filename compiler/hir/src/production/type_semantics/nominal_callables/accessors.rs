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
        let property_value = &self.export.properties[property];
        let HirPropertyIdentity::Ordinary(identity) = &self.export.property_identities[property]
        else {
            return Err(invalid(
                "nominal source accessor belongs to an extension property",
            ));
        };
        let CallableTemplateOrigin::Accessor(accessor) = declaration else {
            return Err(invalid("accessor has another source declaration role"));
        };
        let access = self.access(
            identity.key(),
            DefinitionOriginSubject::PropertyAccessor(accessor),
            visibility,
        )?;
        let parameters = match property_value.owner {
            PropertyOwner::Class(id) => self.export.classes[id].type_params.as_slice(),
            PropertyOwner::Interface(id) => self.export.interfaces[id].type_params.as_slice(),
            PropertyOwner::Struct(id) => self.export.structs[id].type_params.as_slice(),
            PropertyOwner::Enum(id) => self.export.enums[id].type_params.as_slice(),
            PropertyOwner::Object(_) => &[],
            PropertyOwner::TopLevel | PropertyOwner::Extension(_) => {
                return Err(invalid("nominal source accessor has no nominal owner"));
            }
        };
        if crate::production::nominal_interfaces::owner_resolution::from_property(
            self.export,
            property_value.owner,
        ) != Some(owner(&access)?)
        {
            return Err(invalid(
                "nominal source accessor identity has a different owner",
            ));
        }
        resources::binders(self.export, parameters, parameters.len(), self.meter)?;
        let binders = self
            .signatures
            .binder_frame(parameters, 0)
            .map_err(invalid)?;
        resources::ty(self.export, property_value.ty, binders.len(), 3, self.meter)?;
        let value_type = self
            .signatures
            .map_type(property_value.ty, &binders)
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
        let (slots, modality) = match implementation {
            PropertyAccessorImplementation::Body(function)
            | PropertyAccessorImplementation::AbstractSlot(function) => {
                let method = self.export.functions[function]
                    .method
                    .ok_or_else(|| invalid("nominal source accessor body has no member owner"))?;
                self.method_owner(method, owner(&access)?)?;
                (self.slots(method)?, self.modality(function, method)?)
            }
            PropertyAccessorImplementation::Storage => (
                CanonicalProtectedSlotRefsV1::try_new(Vec::new()).map_err(invalid)?,
                CallableModalityV1::Final,
            ),
            PropertyAccessorImplementation::Constant => {
                return Err(invalid("const property cannot export an accessor source"));
            }
        };
        let payload = NominalSourceCallablePayloadV1::try_new(
            declaration,
            owner(&access)?,
            CanonicalBinderListV1::try_new(Vec::new()).map_err(invalid)?,
            CanonicalSourceParameterShapesV1::try_new(parameters).map_err(invalid)?,
            result,
            callable_interfaces::source_accessor_effects(attributes).map_err(invalid)?,
            modality,
            slots,
        )
        .map_err(invalid)?;
        self.push(declaration, access, payload)
    }
}

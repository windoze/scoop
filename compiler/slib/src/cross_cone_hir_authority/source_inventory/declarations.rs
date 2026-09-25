use super::*;
use scoop_hir::{NestedSourceMemberRefV1, NominalSourceShapeV1};

impl Closure<'_, '_> {
    pub(super) fn expand_nominal(&mut self, owner: SourceNominalId) -> Result<(), Error> {
        let record = self
            .world
            .current_interface
            .nominal_interfaces()
            .declaration(owner)
            .ok_or(Error::Missing(Declaration::Nominal(owner)))?;
        let key = self.world.source_nominal_key(owner)?;
        let parent = self.world.source_key_owner("source nominal", &key)?;
        self.owner(parent)?;
        self.binders(record.type_parameters())?;
        for ty in record.exact_supertypes().values() {
            self.signature(ty)?;
        }
        for field in record.source_shape().declared_fields() {
            self.signature(field.value_type())?;
        }
        for constructor in record.declaration_details().constructors().values() {
            self.callable(CallableTemplateOrigin::Constructor(*constructor))?;
        }
        for member in record.declaration_details().members().values() {
            match *member {
                NestedSourceMemberRefV1::Function(id) => {
                    self.callable(CallableTemplateOrigin::Function(id))?
                }
                NestedSourceMemberRefV1::GenericFunction(id) => {
                    self.callable(CallableTemplateOrigin::GenericFunction(id))?
                }
                NestedSourceMemberRefV1::Property(id) => {
                    self.property(PropertyOwner::Property(id))?
                }
            }
        }
        if let NominalSourceShapeV1::Enum(shape) = record.source_shape() {
            for variant in shape.variants() {
                self.callable(CallableTemplateOrigin::VariantConstructor(
                    variant.variant(),
                ))?;
                for field in variant.fields() {
                    self.signature(field.value_type())?;
                }
            }
        }
        for child in record.declaration_details().children().values() {
            self.nominal(*child)?;
        }
        Ok(())
    }

    pub(super) fn expand_callable(&mut self, id: CallableTemplateOrigin) -> Result<(), Error> {
        let interface = self.world.current_interface;
        let record = interface
            .callable_interfaces()
            .declaration(id)
            .ok_or(Error::Missing(Declaration::Callable(id)))?;
        self.owner(record.owner())?;
        self.binders(record.type_parameters())?;
        if let Some(receiver) = record.receiver() {
            self.signature(receiver)?;
        }
        for parameter in record.parameters().parameters() {
            self.signature(parameter.value_type())?;
        }
        self.signature(record.result())?;
        if let CallableTemplateOrigin::Accessor(id) = id {
            let key = self
                .world
                .identities
                .canonical_key::<PersistentPropertyAccessorId, PropertyAccessorKey>(id)
                .map_err(CrossConeHirNominalAuthorityError::Identity)?;
            return self.property(key.owner());
        }

        let protocol = interface
            .source_interfaces()
            .get(id)
            .ok_or(Error::Protocol(id))?;
        for parameter in protocol.parameters().parameters() {
            if let Some(element) = parameter.calling().element_type() {
                self.signature(element)?;
            }
            if let Some(key) = parameter.calling().template() {
                let template = interface
                    .default_templates()
                    .get(key)
                    .ok_or(Error::Default(key))?;
                self.default(template)?;
            }
        }
        Ok(())
    }

    pub(super) fn expand_property(&mut self, id: PropertyDeclarationId) -> Result<(), Error> {
        let record = self
            .world
            .current_interface
            .property_interfaces()
            .declaration(id)
            .ok_or(Error::Missing(Declaration::Property(id)))?;
        self.owner(record.owner())?;
        self.binders(record.type_parameters())?;
        if let Some(receiver) = record.receiver() {
            self.signature(receiver)?;
        }
        self.signature(record.value_type())?;
        for accessor in
            std::iter::once(record.accessors().getter()).chain(record.accessors().setter())
        {
            self.callable(CallableTemplateOrigin::Accessor(accessor))?;
        }
        Ok(())
    }
}

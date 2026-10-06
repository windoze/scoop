use super::*;
use scoop_identity::FieldIdentityView;
use std::collections::BTreeMap;

impl CanonicalCrossConeHirSurfaceAuthority<'_> {
    pub(super) fn validate_class_primary_constructor(
        &self,
        nominal: &NominalInterfaceRecordV1,
    ) -> Result<(), Error> {
        let Some(primary) = nominal.declaration_details().class_primary_constructor() else {
            return Ok(());
        };
        let owner = nominal.declaration();
        let constructor = self
            .current_interface
            .callable_interfaces()
            .declaration(CallableTemplateOrigin::Constructor(primary.constructor()))
            .ok_or_else(|| invalid(owner, "class primary constructor has no callable interface"))?;
        let parameters = constructor.parameters().parameters();
        if parameters.len() != primary.properties().len() {
            return Err(invalid(
                owner,
                "class primary property mapping has a different parameter count",
            ));
        }
        let mut storage = BTreeMap::new();
        for field in nominal.source_shape().declared_fields() {
            let key = self
                .identities
                .canonical_key::<PersistentFieldId, FieldIdentityKey>(field.field())
                .map_err(Error::Identity)?;
            if let FieldIdentityView::SourcePropertyBacking { property, .. } = key.view() {
                storage.insert(property, field);
            }
        }
        for (parameter, property) in parameters.iter().zip(primary.properties()) {
            let Some(property) = property else {
                continue;
            };
            let declaration = self
                .current_interface
                .property_interfaces()
                .declaration(PropertyOwner::Property(*property))
                .ok_or_else(|| invalid(owner, "class primary parameter has no logical property"))?;
            let field = storage.get(property).ok_or_else(|| {
                invalid(
                    owner,
                    "class primary property has no ordinary backing field",
                )
            })?;
            if declaration.value_type() != parameter.value_type()
                || field.value_type() != parameter.value_type()
            {
                return Err(invalid(
                    owner,
                    "class primary property and parameter types differ",
                ));
            }
        }
        Ok(())
    }
}

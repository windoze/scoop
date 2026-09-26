use super::*;

pub(in crate::production::type_semantics) struct Required {
    pub constructors: BTreeSet<PersistentConstructorId>,
    pub callables: BTreeSet<CallableTemplateOrigin>,
    properties: BTreeSet<PersistentPropertyId>,
}
pub(in crate::production::type_semantics) fn collect<'a>(
    nodes: impl Iterator<Item = &'a NominalInterfaceRecordV1>,
) -> Result<Required, Error> {
    let mut required = Required {
        constructors: BTreeSet::new(),
        callables: BTreeSet::new(),
        properties: BTreeSet::new(),
    };
    for node in nodes {
        for id in node.declaration_details().constructors().values() {
            resources::insert(&mut required.constructors, *id)?;
        }
        for member in node.declaration_details().members().values() {
            match member {
                NestedSourceMemberRefV1::Function(id) => resources::insert(
                    &mut required.callables,
                    CallableTemplateOrigin::Function(*id),
                )?,
                NestedSourceMemberRefV1::GenericFunction(id) => resources::insert(
                    &mut required.callables,
                    CallableTemplateOrigin::GenericFunction(*id),
                )?,
                NestedSourceMemberRefV1::Property(id) => {
                    resources::insert(&mut required.properties, *id)?
                }
            }
        }
        if let NominalSourceShapeV1::Enum(shape) = node.source_shape() {
            for variant in shape.variants() {
                resources::insert(
                    &mut required.callables,
                    CallableTemplateOrigin::VariantConstructor(variant.variant()),
                )?;
            }
        }
    }
    Ok(required)
}
impl Required {
    pub(in crate::production::type_semantics) fn properties(
        &self,
    ) -> Result<CanonicalPersistentIdsV1<PersistentPropertyId>, Error> {
        CanonicalPersistentIdsV1::try_new(self.properties.iter().copied().collect())
            .map_err(invalid)
    }
    pub(in crate::production::type_semantics) fn accessors(
        &mut self,
        properties: &[NominalSupportPropertyInterfaceV1],
    ) -> Result<(), Error> {
        for property in properties {
            if let NominalSupportPropertyPayloadV1::Runtime { interface } = property.payload() {
                resources::insert(
                    &mut self.callables,
                    CallableTemplateOrigin::Accessor(interface.getter()),
                )?;
                if let ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } =
                    interface.mutability()
                {
                    resources::insert(
                        &mut self.callables,
                        CallableTemplateOrigin::Accessor(*setter),
                    )?;
                }
            }
        }
        Ok(())
    }
    pub(in crate::production::type_semantics) fn protocols(
        &self,
    ) -> Result<BTreeSet<CallableTemplateOrigin>, Error> {
        let mut required = BTreeSet::new();
        for id in &self.constructors {
            resources::insert(&mut required, CallableTemplateOrigin::Constructor(*id))?;
        }
        for owner in &self.callables {
            if !matches!(owner, CallableTemplateOrigin::Accessor(_)) {
                resources::insert(&mut required, *owner)?;
            }
        }
        Ok(required)
    }
}

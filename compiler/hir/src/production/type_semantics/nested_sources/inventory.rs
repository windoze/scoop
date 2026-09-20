use super::*;

pub(in crate::production::type_semantics) struct Required {
    pub constructors: BTreeSet<PersistentConstructorId>,
    pub callables: BTreeSet<CallableTemplateOrigin>,
    properties: BTreeSet<PersistentPropertyId>,
}
pub(in crate::production::type_semantics) fn collect<'a>(
    nodes: impl Iterator<Item = &'a NominalSourceContractV1>,
    meter: &mut BudgetMeter,
) -> Result<Required, Error> {
    let mut required = Required {
        constructors: BTreeSet::new(),
        callables: BTreeSet::new(),
        properties: BTreeSet::new(),
    };
    for node in nodes {
        meter.charge_nodes(1, &WirePath::root()).map_err(resource)?;
        for id in node.constructors().values() {
            resources::insert(&mut required.constructors, *id, meter)?;
        }
        for member in node.members().values() {
            match member {
                NestedSourceMemberRefV1::Function(id) => resources::insert(
                    &mut required.callables,
                    CallableTemplateOrigin::Function(*id),
                    meter,
                )?,
                NestedSourceMemberRefV1::GenericFunction(id) => resources::insert(
                    &mut required.callables,
                    CallableTemplateOrigin::GenericFunction(*id),
                    meter,
                )?,
                NestedSourceMemberRefV1::Property(id) => {
                    resources::insert(&mut required.properties, *id, meter)?
                }
            }
        }
        if let NominalSourceShapeV1::Enum(shape) = node.source_shape() {
            for variant in shape.variants() {
                resources::insert(
                    &mut required.callables,
                    CallableTemplateOrigin::VariantConstructor(variant.variant()),
                    meter,
                )?;
            }
        }
    }
    Ok(required)
}
impl Required {
    pub(in crate::production::type_semantics) fn properties(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalPersistentIdsV1<PersistentPropertyId>, Error> {
        resources::canonical(self.properties.len(), meter)?;
        CanonicalPersistentIdsV1::try_new(self.properties.iter().copied().collect())
            .map_err(invalid)
    }
    pub(in crate::production::type_semantics) fn accessors(
        &mut self,
        properties: &[NominalSupportPropertyInterfaceV1],
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        for property in properties {
            meter.charge_nodes(1, &WirePath::root()).map_err(resource)?;
            if let NominalSupportPropertyPayloadV1::Runtime { interface } = property.payload() {
                resources::insert(
                    &mut self.callables,
                    CallableTemplateOrigin::Accessor(interface.getter()),
                    meter,
                )?;
                if let ProtectedPropertyMutabilityV1::ReadWrite { setter, .. } =
                    interface.mutability()
                {
                    resources::insert(
                        &mut self.callables,
                        CallableTemplateOrigin::Accessor(*setter),
                        meter,
                    )?;
                }
            }
        }
        Ok(())
    }
    pub(in crate::production::type_semantics) fn protocols(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<BTreeSet<CallableTemplateOrigin>, Error> {
        let mut required = BTreeSet::new();
        for id in &self.constructors {
            resources::insert(
                &mut required,
                CallableTemplateOrigin::Constructor(*id),
                meter,
            )?;
        }
        for owner in &self.callables {
            if !matches!(owner, CallableTemplateOrigin::Accessor(_)) {
                resources::insert(&mut required, *owner, meter)?;
            }
        }
        Ok(required)
    }
}

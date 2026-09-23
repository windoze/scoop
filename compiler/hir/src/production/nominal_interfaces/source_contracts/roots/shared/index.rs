use super::*;

pub(super) struct SourceIndex<'a> {
    pub members: BTreeMap<SourceNominalId, Vec<SourceWork>>,
    protocols: BTreeMap<CallableTemplateOrigin, &'a ExportParameterInterface>,
}

impl<'a> SourceIndex<'a> {
    pub fn new(export: &'a ExportHir, meter: &mut BudgetMeter) -> Result<Self, Error> {
        let mut index = Self {
            members: BTreeMap::new(),
            protocols: BTreeMap::new(),
        };
        for protocol in &export.source_parameter_interfaces {
            work(meter, index.protocols.len())?;
            let Some((id, scope)) = identity::callable(export, protocol.owner)? else {
                continue;
            };
            if scope.provider != export.cone {
                continue;
            }
            meter
                .charge_collection_slots(1, &WirePath::root())
                .map_err(resource)?;
            if index.protocols.insert(id, protocol).is_some() {
                return Err(invalid("source callable has duplicate parameter protocols"));
            }
            if let PublicDeclarationOwnerV1::Nominal(owner) = scope.owner {
                index.member(owner, SourceWork::Callable(id), meter)?;
            }
        }
        for (id, _) in export.properties.iter() {
            work(meter, 1)?;
            let identity = export
                .property_identities
                .get(id)
                .ok_or_else(|| invalid("source property identity is absent"))?;
            let scope = identity::scope(identity.declaration())?;
            if scope.provider == export.cone
                && let PublicDeclarationOwnerV1::Nominal(owner) = scope.owner
            {
                index.member(owner, SourceWork::Property(id), meter)?;
            }
        }
        Ok(index)
    }

    fn member(
        &mut self,
        owner: SourceNominalId,
        value: SourceWork,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        work(meter, self.members.len())?;
        if !self.members.contains_key(&owner) {
            meter
                .charge_collection_slots(1, &WirePath::root())
                .map_err(resource)?;
        }
        push(self.members.entry(owner).or_default(), value, meter)
    }

    pub fn protocol(
        &self,
        id: CallableTemplateOrigin,
        meter: &mut BudgetMeter,
    ) -> Result<&'a ExportParameterInterface, Error> {
        work(meter, self.protocols.len())?;
        self.protocols
            .get(&id)
            .copied()
            .ok_or_else(|| invalid("required source callable has no parameter protocol"))
    }
}

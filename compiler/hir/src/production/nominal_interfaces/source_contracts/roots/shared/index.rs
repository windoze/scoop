use super::*;

pub(super) struct SourceIndex<'a> {
    pub members: BTreeMap<SourceNominalId, Vec<SourceWork>>,
    protocols: BTreeMap<CallableTemplateOrigin, &'a ExportParameterInterface>,
}

impl<'a> SourceIndex<'a> {
    pub fn new(export: &'a ExportHir) -> Result<Self, Error> {
        let mut index = Self {
            members: BTreeMap::new(),
            protocols: BTreeMap::new(),
        };
        for protocol in &export.source_parameter_interfaces {
            let Some((id, scope)) = identity::callable(export, protocol.owner)? else {
                continue;
            };
            if scope.provider != export.cone {
                continue;
            }

            if index.protocols.insert(id, protocol).is_some() {
                return Err(invalid("source callable has duplicate parameter protocols"));
            }
            if let PublicDeclarationOwnerV1::Nominal(owner) = scope.owner {
                index.member(owner, SourceWork::Callable(id))?;
            }
        }
        for (id, _) in export.properties.iter() {
            let identity = export
                .property_identities
                .get(id)
                .ok_or_else(|| invalid("source property identity is absent"))?;
            let scope = identity::scope(identity.declaration())?;
            if scope.provider == export.cone
                && let PublicDeclarationOwnerV1::Nominal(owner) = scope.owner
            {
                index.member(owner, SourceWork::Property(id))?;
            }
        }
        Ok(index)
    }

    fn member(&mut self, owner: SourceNominalId, value: SourceWork) -> Result<(), Error> {
        push(self.members.entry(owner).or_default(), value)
    }

    pub fn protocol(
        &self,
        id: CallableTemplateOrigin,
    ) -> Result<&'a ExportParameterInterface, Error> {
        self.protocols
            .get(&id)
            .copied()
            .ok_or_else(|| invalid("required source callable has no parameter protocol"))
    }
}

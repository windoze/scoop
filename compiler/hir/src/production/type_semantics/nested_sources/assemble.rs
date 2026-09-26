use super::*;

struct Node {
    source: NestedSourceNode,
    support: Vec<NestedSourceSupportV1>,
}
pub(super) struct Assembly {
    nodes: BTreeMap<SourceNominalId, Node>,
}
impl Assembly {
    pub(super) fn new(nodes: Vec<NestedSourceNode>) -> Result<Self, Error> {
        let mut indexed = BTreeMap::new();
        for source in nodes {
            if indexed
                .insert(
                    source.record.declaration(),
                    Node {
                        source,
                        support: Vec::new(),
                    },
                )
                .is_some()
            {
                return Err(invalid("duplicate nested nominal source"));
            }
        }
        Ok(Self { nodes: indexed })
    }
    pub(super) fn push(
        &mut self,
        owner: SourceNominalId,
        record: NestedSourceSupportV1,
    ) -> Result<(), Error> {
        let node = self
            .nodes
            .get_mut(&owner)
            .ok_or_else(|| invalid("nested support belongs to an unselected source owner"))?;
        resources::push(&mut node.support, record)
    }
    pub(super) fn finish(
        mut self,
        root: SourceNominalId,
    ) -> Result<NominalSupportNestedInterfaceV1, Error> {
        let record = self.take(root)?;
        if !self.nodes.is_empty() {
            return Err(invalid(
                "nested source projection contains unused nominal records",
            ));
        }
        Ok(record)
    }
    fn take(&mut self, owner: SourceNominalId) -> Result<NominalSupportNestedInterfaceV1, Error> {
        let mut node = self
            .nodes
            .remove(&owner)
            .ok_or_else(|| invalid("nested source has a missing, repeated or cyclic child"))?;
        for child in node.source.record.declaration_details().children().values() {
            let record = self.take(*child)?;

            resources::push(
                &mut node.support,
                NestedSourceSupportV1::NestedNominal(Box::new(record)),
            )?;
        }

        let support = CanonicalNestedSourceSupportV1::try_new(node.support).map_err(invalid)?;
        let record = &node.source.record;
        let details = record.declaration_details();
        let interface = ProtectedNestedSourceInterfaceV1::try_new(
            record.kind(),
            details.modality(),
            record.type_parameters().clone(),
            record.exact_supertypes().clone(),
            details.constructors().clone(),
            details.members().clone(),
            details.children().clone(),
            record.source_shape().clone(),
            support,
        )
        .map_err(invalid)?;
        let payload =
            ProtectedNestedNominalPayloadV1::try_new(owner, interface).map_err(invalid)?;
        NominalSupportNestedInterfaceV1::try_new(owner, node.source.access, payload)
            .map_err(invalid)
    }
}

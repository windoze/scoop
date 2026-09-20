use super::*;
use scoop_identity::{ExactTypeKey, PersistentExactTypeId};

struct Node {
    source: NestedSourceNode,
    support: Vec<NestedSourceSupportV1>,
}
pub(super) struct Assembly {
    nodes: BTreeMap<SourceNominalId, Node>,
}
impl Assembly {
    pub(super) fn new(
        nodes: Vec<NestedSourceNode>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let mut indexed = BTreeMap::new();
        for source in nodes {
            let path = WirePath::root();
            work(meter, indexed.len())?;
            meter
                .check_table_entries(indexed.len() as u64 + 1, &path)
                .map_err(resource)?;
            meter.charge_collection_slots(1, &path).map_err(resource)?;
            if indexed
                .insert(
                    source.contract.owner(),
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
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        work(meter, self.nodes.len())?;
        let node = self
            .nodes
            .get_mut(&owner)
            .ok_or_else(|| invalid("nested support belongs to an unselected source owner"))?;
        resources::push(&mut node.support, record, meter)
    }
    pub(super) fn finish(
        mut self,
        root: SourceNominalId,
        meter: &mut BudgetMeter,
    ) -> Result<NominalSupportNestedInterfaceV1, Error> {
        let record = self.take(root, meter, 1)?;
        if !self.nodes.is_empty() {
            return Err(invalid(
                "nested source projection contains unused nominal records",
            ));
        }
        Ok(record)
    }
    fn take(
        &mut self,
        owner: SourceNominalId,
        meter: &mut BudgetMeter,
        depth: u64,
    ) -> Result<NominalSupportNestedInterfaceV1, Error> {
        let path = WirePath::root();
        meter.check_semantic_depth(depth, &path).map_err(resource)?;
        meter.charge_nodes(1, &path).map_err(resource)?;
        work(meter, self.nodes.len())?;
        let mut node = self
            .nodes
            .remove(&owner)
            .ok_or_else(|| invalid("nested source has a missing, repeated or cyclic child"))?;
        for child in node.source.contract.children().values() {
            let record = self.take(*child, meter, depth + 1)?;
            resources::boxed(&record, meter)?;
            resources::push(
                &mut node.support,
                NestedSourceSupportV1::NestedNominal(Box::new(record)),
                meter,
            )?;
        }
        resources::canonical(node.support.len(), meter)?;
        let support = CanonicalNestedSourceSupportV1::try_new(node.support).map_err(invalid)?;
        let interface = node
            .source
            .contract
            .into_nested_interface(support)
            .map_err(invalid)?;
        let support = match owner {
            SourceNominalId::Concrete(id) => {
                meter.charge_sha256(40, &path).map_err(resource)?;
                NestedNominalSupportV1::ParamFree {
                    inheritance_exact: PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(id))
                        .map_err(invalid)?,
                    representation_owner: id,
                }
            }
            SourceNominalId::GenericTemplate(_) => NestedNominalSupportV1::GenericTemplate,
        };
        let payload =
            ProtectedNestedNominalPayloadV1::try_new(owner, interface, support).map_err(invalid)?;
        NominalSupportNestedInterfaceV1::try_new(owner, node.source.access, payload)
            .map_err(invalid)
    }
}

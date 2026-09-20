use super::*;
use scoop_identity::PersistentIdResolver;

impl DecodedInterfaceSourceDispatchV1 {
    pub fn resolve<R, E: fmt::Display>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<InterfaceSourceDispatchV1, SourceInventoryError>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
    {
        let path = WirePath::root();
        meter.check_semantic_depth(3, &path)?;
        meter.charge_nodes(1, &path)?;
        meter.charge_work(1, &path)?;
        let mut parents = reserve(self.parents.len(), meter)?;
        let mut members = reserve(self.members.len(), meter)?;
        for (index, member) in self.members.iter().enumerate() {
            member
                .overrides
                .charge_resolution_at(meter, &path.clone().field(3).index(index as u64).field(2))?;
        }
        meter.charge_edges(
            1 + self.parents.len() as u64 + self.members.len() as u64,
            &path,
        )?;
        let owner = resolver.resolve(self.owner).map_err(reference)?;
        for parent in self.parents {
            parents.push(resolver.resolve(parent).map_err(reference)?);
        }
        for member in self.members {
            let slot = resolver.resolve(member.slot).map_err(reference)?;
            let overrides = member.overrides.resolve(resolver).map_err(reference)?;
            members.push(InterfaceSourceMemberV1::new(slot, overrides));
        }
        InterfaceSourceDispatchV1::try_new(owner, parents, members, meter)
    }
}
impl DecodedCanonicalInterfaceSourceDispatchesV1 {
    pub fn resolve<R, E: fmt::Display>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalInterfaceSourceDispatchesV1, SourceInventoryError>
    where
        R: PersistentIdResolver<PersistentExactTypeId, Error = E>
            + PersistentIdResolver<PersistentDispatchSlotId, Error = E>,
    {
        let mut records = reserve(self.records.len(), meter)?;
        for record in self.records {
            records.push(record.resolve(resolver, meter)?);
        }
        CanonicalInterfaceSourceDispatchesV1::from_ordered(records, meter)
    }
}

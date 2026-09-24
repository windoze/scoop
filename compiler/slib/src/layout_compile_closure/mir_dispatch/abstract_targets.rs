use super::*;
use scoop_identity::CallableTemplateOrigin;

type Targets =
    BTreeMap<(hir::SourceNominalId, PersistentDispatchSlotId), StrongCallableDefinitionOwner>;

pub(super) fn collect(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dependencies: &[hir::CheckedSharedTypeFoundationV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<Targets, Error> {
    let mut targets = BTreeMap::new();
    for provider in std::iter::once(source).chain(dependencies.iter().copied()) {
        for declaration in provider
            .metadata()
            .public
            .callable_interfaces()
            .all_declarations()
        {
            meter.charge_work(1, &WirePath::root())?;
            if declaration.modality() != hir::CallableModalityV1::Abstract {
                continue;
            }
            let Some(owner) = declaration.owner().nominal_owner() else {
                continue;
            };
            let target = match declaration.declaration() {
                CallableTemplateOrigin::Function(id) => StrongCallableDefinitionOwner::Function(id),
                CallableTemplateOrigin::Accessor(id) => {
                    StrongCallableDefinitionOwner::PropertyAccessor(id)
                }
                CallableTemplateOrigin::GenericFunction(_)
                | CallableTemplateOrigin::Constructor(_)
                | CallableTemplateOrigin::VariantConstructor(_) => continue,
            };
            for slot in declaration.slot_relations().values() {
                lookup(targets.len(), meter)?;
                meter.check_table_entries(targets.len() as u64 + 1, &WirePath::root())?;
                meter.charge_collection_slots(1, &WirePath::root())?;
                if targets.insert((owner, *slot), target).is_some() {
                    return Err(Error::AbstractSlot { owner, slot: *slot });
                }
            }
        }
    }
    Ok(targets)
}

impl Replay<'_, '_> {
    pub(super) fn abstract_target(
        &self,
        owner: PersistentExactTypeId,
        contract: &hir::InheritanceSlotContractV1,
        meter: &mut BudgetMeter,
    ) -> Result<(StrongCallableDefinitionOwner, PersistentExactTypeId), Error> {
        let mut current = owner;
        let mut depth = 0;
        loop {
            depth += 1;
            meter.check_semantic_depth(depth, &WirePath::root())?;
            lookup(self.graph.node_count(), meter)?;
            let node = self.graph.get(current).ok_or(Error::Missing(current))?;
            if node.edges().modality() == hir::NominalInheritanceModalityV1::Interface {
                break;
            }
            lookup(self.abstract_targets.len(), meter)?;
            if let Some(target) = self.abstract_targets.get(&(node.source(), contract.slot())) {
                return Ok((*target, current));
            }
            match node.edges().direct_base() {
                hir::DirectClassBaseV1::NoClassBase => break,
                hir::DirectClassBaseV1::ClassBase { exact } => current = exact,
            }
        }
        let receiver = contract
            .signature()
            .exact_signature()
            .receiver()
            .into_option()
            .ok_or(Error::SourceSlot {
                owner,
                slot: contract.slot(),
            })?;
        Ok((target(contract.declaration()), receiver))
    }
}

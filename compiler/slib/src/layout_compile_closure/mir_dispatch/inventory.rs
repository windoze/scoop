use super::*;
use scoop_identity::{CoreBuiltinNominal, SignatureTypeKey};

pub(super) fn validate(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dispatch: &mir::CanonicalMirDispatchSchemasV1,
    replay: &mut Replay<'_, '_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut required = BTreeSet::new();
    let facts = source.facts();
    for nominal in source.section().inheritance().records() {
        lookup(facts.records().len(), meter)?;
        let fact = facts
            .get(nominal.owner())
            .ok_or(Error::MissingFact(nominal.owner()))?;
        let value = matches!(fact.kind(), hir::ExactTypeKindV1::Value { .. });
        let record = get(dispatch, nominal.owner(), meter)?;
        replay.record(nominal, value, record, meter)?;
        insert(&mut required, nominal.owner(), meter)?;
    }
    for representation in source.representations().table().records() {
        meter.charge_work(1, &WirePath::root())?;
        if let hir::NominalRepresentationShapeV1::Object { backing_class, .. } =
            representation.shape()
        {
            let metadata = source.metadata();
            let exact = metadata
                .signature_exact_type(&SignatureTypeKey::Nominal(representation.owner()), meter)?;
            let backing =
                metadata.signature_exact_type(&SignatureTypeKey::Nominal(*backing_class), meter)?;
            lookup(source.section().inheritance().records().len(), meter)?;
            let nominal = source
                .section()
                .inheritance()
                .get(exact)
                .ok_or(Error::Missing(exact))?;
            replay.record(nominal, false, get(dispatch, backing, meter)?, meter)?;
            insert(&mut required, backing, meter)?;
        }
    }
    let any = source.metadata().signature_exact_type(
        &SignatureTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id()),
        meter,
    )?;
    lookup(source.facts().records().len(), meter)?;
    if source.facts().get(any).is_some() {
        let record = get(dispatch, any, meter)?;
        Error::schema(
            any,
            Component::Tables,
            matches!(record.vtable(), mir::MirClassVtableSchemaV1::ClassVtable(entries) if entries.is_empty())
                && record.itables().is_empty(),
        )?;
        insert(&mut required, any, meter)?;
    }
    for record in dispatch.records() {
        lookup(required.len(), meter)?;
        if !required.contains(&record.owner()) {
            return Err(Error::Unexpected(record.owner()));
        }
    }
    Ok(())
}

fn get<'a>(
    table: &'a mir::CanonicalMirDispatchSchemasV1,
    owner: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<&'a mir::ParamFreeMirDispatchSchemaV1, Error> {
    lookup(table.records().len(), meter)?;
    table.get(owner).ok_or(Error::Missing(owner))
}

impl Replay<'_, '_> {
    pub(super) fn record(
        &mut self,
        source: &hir::NominalInheritanceInterfaceV1,
        value: bool,
        candidate: &mir::ParamFreeMirDispatchSchemaV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let expected = source.slot_schemas().records();
        meter.charge_work(
            expected.len() as u64 + candidate.itables().len() as u64 + 1,
            &WirePath::root(),
        )?;
        let class = source
            .slot_schemas()
            .get(hir::InheritanceSlotSchemaRoleV1::ClassVtable);
        Error::schema(
            candidate.owner(),
            Component::Tables,
            class.is_some()
                == matches!(
                    candidate.vtable(),
                    mir::MirClassVtableSchemaV1::ClassVtable(_)
                ),
        )?;
        if let Some(schema) = class {
            self.table(
                source,
                schema,
                value,
                candidate.owner(),
                candidate.vtable().entries(),
                meter,
            )?;
        }
        let interfaces = expected.iter().filter_map(|schema| match schema.role() {
            hir::InheritanceSlotSchemaRoleV1::ClassVtable => None,
            hir::InheritanceSlotSchemaRoleV1::Interface { interface_exact } => {
                Some((interface_exact, schema))
            }
        });
        Error::schema(
            candidate.owner(),
            Component::Tables,
            interfaces.clone().count() == candidate.itables().len(),
        )?;
        for ((interface, schema), table) in interfaces.zip(candidate.itables()) {
            Error::schema(
                candidate.owner(),
                Component::Tables,
                interface == table.interface(),
            )?;
            self.table(
                source,
                schema,
                value,
                candidate.owner(),
                table.entries(),
                meter,
            )?;
        }
        Ok(())
    }

    fn table(
        &mut self,
        source: &hir::NominalInheritanceInterfaceV1,
        schema: &hir::InheritanceSlotSchemaV1,
        value: bool,
        owner: PersistentExactTypeId,
        entries: &[mir::MirDispatchEntryV1],
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        meter.charge_work(
            schema.slots().len() as u64 + entries.len() as u64,
            &WirePath::root(),
        )?;
        Error::schema(
            owner,
            Component::Slots,
            schema.slots().len() == entries.len(),
        )?;
        for (position, (slot, entry)) in schema.slots().iter().zip(entries).enumerate() {
            Error::entry(owner, *slot, Component::SlotIdentity, entry.slot() == *slot)?;
            Error::entry(
                owner,
                *slot,
                Component::Position,
                entry.position().get() as usize == position,
            )?;
            lookup(source.slots().records().len(), meter)?;
            let contract = source
                .slots()
                .get(*slot)
                .ok_or(Error::SourceSlot { owner, slot: *slot })?;
            self.entry(
                (source.owner(), value),
                schema.role(),
                contract,
                entry,
                meter,
            )?;
        }
        Ok(())
    }
}

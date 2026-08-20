use super::*;
use scoop_identity::SignatureTypeKey;

pub(super) fn validate(
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    dispatch: &mir::CanonicalMirDispatchSchemasV1,
    replay: &mut Replay<'_>,
) -> Result<(), Error> {
    let mut required = BTreeSet::new();
    let facts = source.facts();
    for nominal in source.section().inheritance().records() {
        let fact = facts
            .get(nominal.owner())
            .ok_or(Error::MissingFact(nominal.owner()))?;
        let value = matches!(fact.kind(), hir::ExactTypeKindV1::Value { .. });
        let record = get(dispatch, nominal.owner())?;
        replay.record(nominal, value, record)?;
        insert(&mut required, nominal.owner())?;
    }
    for representation in source.representations().table().records() {
        if let hir::NominalRepresentationShapeV1::Object { backing_class, .. } =
            representation.shape()
        {
            let metadata = source.metadata();
            let exact = metadata
                .signature_exact_type(&SignatureTypeKey::Nominal(representation.owner()))?;
            let backing =
                metadata.signature_exact_type(&SignatureTypeKey::Nominal(*backing_class))?;

            let nominal = source
                .section()
                .inheritance()
                .get(exact)
                .ok_or(Error::Missing(exact))?;
            replay.record(nominal, false, get(dispatch, backing)?)?;
            insert(&mut required, backing)?;
        }
    }
    for record in dispatch.records() {
        if !required.contains(&record.owner()) {
            let metadata = source.metadata();
            let key = metadata
                .identities
                .canonical_key::<_, scoop_identity::ExactTypeKey>(record.owner())?;
            if let scoop_identity::ExactTypeKey::Nominal(nominal) = key.as_ref()
                && matches!(
                    metadata
                        .identities
                        .canonical_key::<_, scoop_identity::GeneratedNominalKey>(*nominal)?
                        .as_ref(),
                    scoop_identity::GeneratedNominalKey::TaskContext(_)
                )
            {
                Error::schema(
                    record.owner(),
                    Component::Tables,
                    record.vtable().is_empty()
                        && record.itables().is_empty()
                        && record
                            .interface_slots()
                            .is_none_or(|slots| slots.is_empty()),
                )?;
                continue;
            }
            let structural_box =
                if let scoop_identity::ExactTypeKey::Nominal(nominal) = key.as_ref() {
                    let generated = metadata
                        .identities
                        .canonical_key::<_, scoop_identity::GeneratedNominalKey>(*nominal)?;
                    if let scoop_identity::GeneratedNominalKey::BoxedValue { payload } =
                        generated.as_ref()
                    {
                        matches!(
                            metadata
                                .identities
                                .canonical_key::<_, scoop_identity::ExactTypeKey>(*payload)?
                                .as_ref(),
                            scoop_identity::ExactTypeKey::Tuple(_)
                                | scoop_identity::ExactTypeKey::RawPointer(_)
                        )
                    } else {
                        false
                    }
                } else {
                    false
                };
            if !structural_box
                && !matches!(
                    key.as_ref(),
                    scoop_identity::ExactTypeKey::NominalApplication { .. }
                )
            {
                return Err(Error::Unexpected(record.owner()));
            }
            for entry in record
                .vtable()
                .iter()
                .chain(record.itables().iter().flat_map(|table| table.entries()))
            {
                if let Some(binding) = replay.callables.get(entry.implementation().target())
                    && let mir::MirCallableOriginV1::Generated {
                        callable,
                        role:
                            GeneratedCallableKey::BoxingAdjust { .. }
                            | GeneratedCallableKey::DispatchAdjust { .. },
                    } = binding.origin().as_ref()
                {
                    replay.adjustments.insert(*callable);
                }
            }
        }
    }
    Ok(())
}

fn get(
    table: &mir::CanonicalMirDispatchSchemasV1,
    owner: PersistentExactTypeId,
) -> Result<&mir::ParamFreeMirDispatchSchemaV1, Error> {
    table.get(owner).ok_or(Error::Missing(owner))
}

impl Replay<'_> {
    pub(super) fn record(
        &mut self,
        source: &hir::NominalInheritanceInterfaceV1,
        value: bool,
        candidate: &mir::ParamFreeMirDispatchSchemaV1,
    ) -> Result<(), Error> {
        let expected = source.slot_schemas().records();
        if let Some(slots) = candidate.interface_slots() {
            let role = hir::InheritanceSlotSchemaRoleV1::Interface {
                interface_exact: source.owner(),
            };
            Error::schema(
                candidate.owner(),
                Component::Tables,
                expected.len() == 1 && expected[0].role() == role,
            )?;
            let schema = &expected[0];
            Error::schema(
                candidate.owner(),
                Component::Slots,
                schema.slots().len() == slots.len(),
            )?;
            for (position, (slot, candidate)) in schema.slots().iter().zip(slots).enumerate() {
                Error::entry(
                    source.owner(),
                    *slot,
                    Component::SlotIdentity,
                    candidate.slot() == *slot,
                )?;
                Error::entry(
                    source.owner(),
                    *slot,
                    Component::Position,
                    candidate.position().get() as usize == position,
                )?;
                let contract =
                    source
                        .slots()
                        .get(schema.role(), *slot)
                        .ok_or(Error::SourceSlot {
                            owner: source.owner(),
                            slot: *slot,
                        })?;
                let signature = bindings::signature(contract.signature(), Some(source.owner()))?;
                Error::entry(
                    source.owner(),
                    *slot,
                    Component::Signature,
                    candidate.signature() == &self.lowered_signature(&signature)?,
                )?;
            }
            return Ok(());
        }

        let class = source
            .slot_schemas()
            .get(hir::InheritanceSlotSchemaRoleV1::ClassVtable);
        Error::schema(
            candidate.owner(),
            Component::Tables,
            class.is_some() == matches!(candidate.slots(), mir::MirDispatchSlotsV1::ClassVtable(_)),
        )?;
        if let Some(schema) = class {
            self.table(source, schema, value, candidate.owner(), candidate.vtable())?;
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
            self.table(source, schema, value, candidate.owner(), table.entries())?;
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
    ) -> Result<(), Error> {
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

            let contract = source
                .slots()
                .get(schema.role(), *slot)
                .ok_or(Error::SourceSlot { owner, slot: *slot })?;
            self.entry((source.owner(), value), schema.role(), contract, entry)?;
        }
        Ok(())
    }
}

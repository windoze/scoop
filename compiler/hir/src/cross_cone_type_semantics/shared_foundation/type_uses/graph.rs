use super::*;

impl<'a> Graph<'a> {
    pub(super) fn new(
        current: SharedTypeMetadataV1<'a>,
        dependencies: &[SharedTypeMetadataV1<'a>],
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let mut result = Self {
            current,
            providers: BTreeMap::new(),
            requirements: BTreeMap::new(),
            expanded: BTreeSet::new(),
            pending: Vec::new(),
            selected: BTreeSet::new(),
        };
        result.provider(current, meter)?;
        for dependency in dependencies {
            if dependency.provider == current.provider {
                return Err(Error::CurrentProviderDependency(dependency.provider));
            }
            result.provider(*dependency, meter)?;
        }
        Ok(result)
    }

    fn provider(
        &mut self,
        metadata: SharedTypeMetadataV1<'a>,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        meter.charge_work(1 + u64::from(self.providers.len().max(1).ilog2()), &path)?;
        if self.providers.contains_key(&metadata.provider) {
            return Err(Error::DuplicateProvider(metadata.provider));
        }
        meter.check_table_entries(self.providers.len() as u64 + 1, &path)?;
        meter.charge_collection_slots(1, &path)?;
        let nominals = metadata.public.nominal_interfaces();
        let callables = metadata.public.callable_interfaces();
        let materialization =
            NominalMaterializationClosure::from_declarations(nominals, callables, meter)?;
        for declaration in nominals.all_records() {
            let SourceNominalId::Concrete(owner) = declaration.declaration() else {
                continue;
            };
            meter.charge_work(
                1 + u64::from(self.current.identities.identity_count().max(1).ilog2()),
                &path,
            )?;
            let key = self
                .current
                .identities
                .canonical_key::<_, SourceDeclarationKey>(owner)?;
            if key.origin() != metadata.provider {
                return Err(Error::NominalOwner(owner));
            }
        }
        nominals.visit_materialization_requirements(callables, meter, |requirement, meter| {
            let owner = requirement.owner();
            meter.charge_work(1 + u64::from(self.requirements.len().max(1).ilog2()), &path)?;
            if !self.requirements.contains_key(&owner) {
                meter.check_table_entries(self.requirements.len() as u64 + 1, &path)?;
                meter.charge_collection_slots(1, &path)?;
            }
            let requirements = self.requirements.entry(owner).or_default();
            meter.check_table_entries(requirements.len() as u64 + 1, &path)?;
            meter.try_reserve_collection_slots(requirements, 1, &path)?;
            requirements.push(requirement);
            Ok::<_, Error>(())
        })?;
        self.providers.insert(
            metadata.provider,
            Provider {
                metadata,
                materialization,
            },
        );
        Ok(())
    }

    pub(super) fn select(
        &mut self,
        owner: PersistentTypeId,
        kind: Kind,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        meter.charge_edges(if kind == Kind::Representation { 1 } else { 2 }, &path)?;
        let (provider, exact) = self.resolve_nominal(owner, meter)?;
        if provider != self.current.provider {
            self.record((provider, kind, exact), meter)?;
            self.record((provider, Kind::Representation, exact), meter)?;
        }
        meter.charge_work(1 + u64::from(self.expanded.len().max(1).ilog2()), &path)?;
        if !self.expanded.contains(&owner) {
            meter.check_table_entries(self.expanded.len() as u64 + 1, &path)?;
            meter.charge_collection_slots(1, &path)?;
            meter.try_reserve_collection_slots(&mut self.pending, 1, &path)?;
            self.expanded.insert(owner);
            self.pending.push(owner);
        }
        Ok(())
    }

    fn record(
        &mut self,
        record: (ConeIdentity, Kind, PersistentExactTypeId),
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let path = WirePath::root().field(8);
        meter.charge_work(1 + u64::from(self.selected.len().max(1).ilog2()), &path)?;
        if !self.selected.contains(&record) {
            meter.check_table_entries(self.selected.len() as u64 + 1, &path)?;
            meter.charge_collection_slots(1, &path)?;
            self.selected.insert(record);
        }
        Ok(())
    }

    pub(super) fn finish(
        self,
        meter: &mut BudgetMeter,
    ) -> Result<CanonicalSelectedExternalTypeUsesV1, Error> {
        let path = WirePath::root().field(8);
        let mut records = Vec::new();
        meter.try_reserve_collection_slots(&mut records, self.selected.len(), &path)?;
        for (provider, kind, exact) in self.selected {
            records.push(SelectedExternalTypeUseV1::new(provider, kind.usage(exact)));
        }
        let bytes = records
            .iter()
            .try_fold(0_u64, |bytes, record| {
                scoop_wire::encoded_length(record).map(|size| bytes.saturating_add(size))
            })
            .map_err(|e| Error::Key(e.to_string()))?;
        meter.charge_owned_bytes(bytes, &path)?;
        meter.charge_collection_slots((records.len() as u64).saturating_mul(2), &path)?;
        meter.charge_work(
            bytes.saturating_mul(2 + u64::from(records.len().max(1).ilog2())),
            &path,
        )?;
        CanonicalSelectedExternalTypeUsesV1::try_new(records).map_err(|e| Error::Key(e.to_string()))
    }
}

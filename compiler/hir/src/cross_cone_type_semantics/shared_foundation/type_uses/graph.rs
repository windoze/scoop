use super::*;

impl<'a> Graph<'a> {
    pub(super) fn new(
        current: SharedTypeMetadataV1<'a>,
        dependencies: &[SharedTypeMetadataV1<'a>],
    ) -> Result<Self, Error> {
        let mut result = Self {
            current,
            providers: BTreeMap::new(),
            requirements: BTreeMap::new(),
            expanded: BTreeSet::new(),
            pending: Vec::new(),
            selected: BTreeSet::new(),
        };
        result.provider(current)?;
        for dependency in dependencies {
            if dependency.provider == current.provider {
                return Err(Error::CurrentProviderDependency(dependency.provider));
            }
            result.provider(*dependency)?;
        }
        Ok(result)
    }

    fn provider(&mut self, metadata: SharedTypeMetadataV1<'a>) -> Result<(), Error> {
        let path = WirePath::root().field(8);

        if self.providers.contains_key(&metadata.provider) {
            return Err(Error::DuplicateProvider(metadata.provider));
        }

        let nominals = metadata.public.nominal_interfaces();
        let callables = metadata.public.callable_interfaces();
        let materialization =
            NominalMaterializationClosure::from_declarations(nominals, callables)?;
        for declaration in nominals.all_records() {
            let SourceNominalId::Concrete(owner) = declaration.declaration() else {
                continue;
            };

            let key = self
                .current
                .identities
                .canonical_key::<_, SourceDeclarationKey>(owner)?;
            if key.origin() != metadata.provider {
                return Err(Error::NominalOwner(owner));
            }
        }
        nominals.visit_materialization_requirements(callables, |requirement| {
            let owner = requirement.owner();

            let requirements = self.requirements.entry(owner).or_default();

            scoop_wire::allocation::try_reserve(requirements, 1, &path)?;
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

    pub(super) fn select(&mut self, owner: PersistentTypeId, kind: Kind) -> Result<(), Error> {
        let path = WirePath::root().field(8);

        let (provider, exact) = self.resolve_nominal(owner)?;
        if provider != self.current.provider {
            self.record((provider, kind, exact))?;
            self.record((provider, Kind::Representation, exact))?;
        }

        if !self.expanded.contains(&owner) {
            scoop_wire::allocation::try_reserve(&mut self.pending, 1, &path)?;
            self.expanded.insert(owner);
            self.pending.push(owner);
        }
        Ok(())
    }

    fn record(&mut self, record: (ConeIdentity, Kind, PersistentExactTypeId)) -> Result<(), Error> {
        if !self.selected.contains(&record) {
            self.selected.insert(record);
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Result<CanonicalSelectedExternalTypeUsesV1, Error> {
        let path = WirePath::root().field(8);
        let mut records = Vec::new();
        scoop_wire::allocation::try_reserve(&mut records, self.selected.len(), &path)?;
        for (provider, kind, exact) in self.selected {
            records.push(SelectedExternalTypeUseV1::new(provider, kind.usage(exact)));
        }

        CanonicalSelectedExternalTypeUsesV1::try_new(records).map_err(|e| Error::Key(e.to_string()))
    }
}

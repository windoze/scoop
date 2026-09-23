use super::*;

/// A provider's bound transcript joined to its independently checked public API.
#[derive(Clone, Copy, Debug)]
pub struct TypeFoundationSourceProviderV1<'a> {
    pub(super) source: &'a BoundTypeFoundationSourcesV1<'a>,
    pub(super) public: CheckedTypeSectionPublicSupportV1<'a>,
}

impl<'a> TypeFoundationSourceProviderV1<'a> {
    pub fn try_new(
        source: &'a BoundTypeFoundationSourcesV1<'a>,
        public: CheckedTypeSectionPublicSupportV1<'a>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, TypeFoundationReplayError> {
        if source.source.entries().provider != public.provider() {
            return Err(TypeFoundationReplayError::PublicProvider {
                source: source.source.entries().provider,
                public: public.provider(),
            });
        }
        validate_public(source, public.section().nominal_interfaces(), meter)?;
        Ok(Self { source, public })
    }

    pub fn provider(self) -> ConeIdentity {
        self.source.source.entries().provider
    }

    pub(super) fn entries(self) -> &'a TypeFoundationSourceEntriesV1 {
        self.source.source.entries()
    }

    pub(super) fn representation(
        self,
        owner: PersistentTypeId,
    ) -> Result<NominalRepresentationSourceV1<'a>, TypeFoundationReplayError> {
        let nominal = SourceNominalId::Concrete(owner);
        let representation = self
            .entries()
            .representations
            .get(owner)
            .ok_or(TypeFoundationReplayError::MissingRepresentation(owner))?;
        let public_source_shape = self
            .public
            .section()
            .nominal_interfaces()
            .get(nominal)
            .map_or(
                NominalRepresentationPublicSourceShapeV1::NoPublicSourceShape,
                |record| {
                    NominalRepresentationPublicSourceShapeV1::PublicSourceShape(
                        record.source_shape(),
                    )
                },
            );
        Ok(NominalRepresentationSourceV1 {
            key: self.source.nominal_key(nominal)?,
            access: self.source.nominal_source(nominal)?.access(),
            shape: representation.shape(),
            public_source_shape,
        })
    }
}

pub(super) fn validate_public(
    source: &BoundTypeFoundationSourcesV1<'_>,
    public: &CanonicalNominalInterfacesV1,
    meter: &mut BudgetMeter,
) -> Result<(), TypeFoundationReplayError> {
    let path = WirePath::root();
    meter.check_semantic_depth(1, &path)?;
    meter.charge_nodes(1, &path)?;
    meter.check_table_entries(public.records().len() as u64, &path)?;
    let entries = source.source.entries();
    for (index, record) in public.records().iter().enumerate() {
        let at = path.clone().index(index as u64);
        meter.charge_work(u64::from(source.nominal_keys.len().max(1).ilog2()) + 1, &at)?;
        if !source.nominal_keys.contains_key(&record.declaration()) {
            return Err(TypeFoundationReplayError::PublicNominal(
                record.declaration(),
            ));
        }
        if let SourceNominalId::Concrete(owner) = record.declaration() {
            meter.charge_work(
                u64::from(entries.representations.records().len().max(1).ilog2()) + 1,
                &at,
            )?;
            let representation = entries
                .representations
                .get(owner)
                .ok_or(TypeFoundationReplayError::MissingRepresentation(owner))?;
            if !representation.public_source_shape_matches(record.source_shape(), meter, &at)? {
                return Err(TypeFoundationReplayError::PublicSourceShape(owner));
            }
        }
    }
    Ok(())
}

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
    ) -> Result<Self, TypeFoundationReplayError> {
        if source.source.entries().provider != public.provider() {
            return Err(TypeFoundationReplayError::PublicProvider {
                source: source.source.entries().provider,
                public: public.provider(),
            });
        }
        validate_public(source, public.section().nominal_interfaces())?;
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
) -> Result<(), TypeFoundationReplayError> {
    let path = WirePath::root();

    let entries = source.source.entries();
    for (index, record) in public.records().iter().enumerate() {
        let at = path.clone().index(index as u64);

        if !source.nominal_keys.contains_key(&record.declaration()) {
            return Err(TypeFoundationReplayError::PublicNominal(
                record.declaration(),
            ));
        }
        if let SourceNominalId::Concrete(owner) = record.declaration() {
            if entries
                .representation_owners
                .values()
                .binary_search(&owner)
                .is_err()
            {
                continue;
            }

            let representation = entries
                .representations
                .get(owner)
                .ok_or(TypeFoundationReplayError::MissingRepresentation(owner))?;
            if !representation.public_source_shape_matches(record.source_shape(), &at)? {
                return Err(TypeFoundationReplayError::PublicSourceShape(owner));
            }
        }
    }
    Ok(())
}

use super::*;

pub(super) fn validate<E>(
    exports: &MirTypeBridgeExportConstituentsV1,
    provider: ConeIdentity,
    identities: &ValidatedIdentityGraph,
    sources: &[PersistentTypeId],
) -> Result<(), MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;

    if sources.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::NonCanonicalInventory(
            MirTypeBridgeSourceInventoryV1::SourceRoots,
        ));
    }
    for source in sources {
        let key = identities
            .canonical_key::<_, SourceDeclarationKey>(*source)
            .map_err(Error::Reference)?;
        if key.origin() != provider {
            return Err(Error::SourceRootProvider { source: *source });
        }
        let exact = exact(*source)?;
        if !exports
            .types
            .get(exact)
            .is_some_and(|record| record.origin() == &MirTypeOriginV1::SourceNominal(*source))
        {
            return Err(Error::SourceRootType { source: *source });
        }
    }
    exports
        .shapes
        .validate_required_sources(sources)
        .map_err(Error::Shape)?;
    let authority = MirShapeSupportAuthority {
        identities,
        types: &exports.types,
    };
    for record in exports.shapes.records() {
        authority.validate(record).map_err(Error::Shape)?;
    }
    Ok(())
}

fn exact<E>(
    nominal: PersistentTypeId,
) -> Result<PersistentExactTypeId, MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    let key = ExactTypeKey::Nominal(nominal);

    PersistentExactTypeId::from_key(&key).map_err(Error::ExactIdentity)
}

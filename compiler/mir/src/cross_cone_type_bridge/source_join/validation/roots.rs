use super::*;

pub(super) fn validate<E>(
    exports: &MirTypeBridgeExportConstituentsV1,
    provider: ConeIdentity,
    identities: &ValidatedIdentityGraph,
    authority: MirTypeBridgeShapeRootAuthorityV1<'_>,
    sources: &[PersistentTypeId],
    meter: &mut BudgetMeter,
) -> Result<(), MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    meter
        .check_table_entries(sources.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    meter
        .charge_work(sources.len() as u64, &WirePath::root())
        .map_err(Error::Resource)?;
    if sources.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(Error::NonCanonicalInventory(
            MirTypeBridgeSourceInventoryV1::SourceRoots,
        ));
    }
    for source in sources {
        meter
            .charge_work(2, &WirePath::root())
            .map_err(Error::Resource)?;
        let key = identities
            .canonical_key::<_, SourceDeclarationKey>(*source)
            .map_err(Error::Reference)?;
        if key.origin() != provider {
            return Err(Error::SourceRootProvider { source: *source });
        }
        let exact = exact(*source, meter)?;
        if !exports
            .types
            .get(exact)
            .is_some_and(|record| record.origin() == &MirTypeOriginV1::SourceNominal(*source))
        {
            return Err(Error::SourceRootType { source: *source });
        }
    }
    match (provider, authority) {
        (ConeIdentity::CORE, MirTypeBridgeShapeRootAuthorityV1::Core(bridge)) => {
            if !exports.shapes.records().is_empty()
                || !bridge
                    .shape_support_roots()
                    .iter()
                    .map(|root| root.source())
                    .eq(sources.iter().copied())
            {
                return Err(Error::CoreRootAuthority);
            }
            for root in bridge.shape_support_roots() {
                core_family(exports, identities, root.source(), root.exact(), meter)?;
            }
        }
        (ConeIdentity::CORE, MirTypeBridgeShapeRootAuthorityV1::Ordinary)
        | (_, MirTypeBridgeShapeRootAuthorityV1::Core(_)) => return Err(Error::CoreRootAuthority),
        (_, MirTypeBridgeShapeRootAuthorityV1::Ordinary) => {
            exports
                .shapes
                .validate_required_sources(sources, meter)
                .map_err(Error::Shape)?;
            let authority = MirShapeSupportAuthority {
                identities,
                types: &exports.types,
            };
            for record in exports.shapes.records() {
                authority.validate(record, meter).map_err(Error::Shape)?;
            }
        }
    }
    Ok(())
}

pub(in crate::cross_cone_type_bridge) fn core_family<E>(
    exports: &MirTypeBridgeExportConstituentsV1,
    identities: &ValidatedIdentityGraph,
    source: PersistentTypeId,
    exact: PersistentExactTypeId,
    meter: &mut BudgetMeter,
) -> Result<ParamFreeMirShapeSupportV1, MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    let source_type = exports
        .types
        .get(exact)
        .ok_or(Error::SourceRootType { source })?;
    let boxed = if source_type.facts().kind() == MirValueKindV1::Reference {
        MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox
    } else {
        MirBoxedShapeSupportV1::Available(helper(
            GeneratedNominalKey::BoxedValue { payload: exact },
            meter,
        )?)
    };
    let step = helper(GeneratedNominalKey::CoroutineStep { result: exact }, meter)?;
    let slot = helper(GeneratedNominalKey::CoroutineSlot { value: exact }, meter)?;
    ParamFreeMirShapeSupportV1::try_new(
        MirShapeSupportAuthority {
            identities,
            types: &exports.types,
        },
        source,
        exact,
        boxed,
        step,
        slot,
        meter,
    )
    .map_err(Error::Shape)
}
fn helper<E>(
    key: GeneratedNominalKey,
    meter: &mut BudgetMeter,
) -> Result<PersistentExactTypeId, MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    let work = scoop_wire::encoded_length(&key).map_err(Error::Encoding)?;
    meter
        .charge_work(work, &WirePath::root())
        .map_err(Error::Resource)?;
    let nominal = PersistentTypeId::from_generated_key(&key).map_err(Error::GeneratedIdentity)?;
    exact(nominal, meter)
}
fn exact<E>(
    nominal: PersistentTypeId,
    meter: &mut BudgetMeter,
) -> Result<PersistentExactTypeId, MirTypeBridgeSourceJoinError<E>> {
    use MirTypeBridgeSourceJoinError as Error;
    let key = ExactTypeKey::Nominal(nominal);
    let work = scoop_wire::encoded_length(&key).map_err(Error::Encoding)?;
    meter
        .charge_work(work, &WirePath::root())
        .map_err(Error::Resource)?;
    PersistentExactTypeId::from_key(&key).map_err(Error::ExactIdentity)
}

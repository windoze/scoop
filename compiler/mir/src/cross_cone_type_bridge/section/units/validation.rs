use super::*;

pub(super) fn unit<E>(
    authority: MirTypeBridgeLocalAuthorityV1<'_>,
    source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
    unit: PersistentInitializationUnitId,
    graph: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<MirTypeBridgeInitializationUnitV1, MirTypeBridgeSectionError<E>> {
    use MirTypeBridgeSectionError as Error;
    meter.charge_nodes(1, &WirePath::root())?;
    meter.charge_work(4, &WirePath::root())?;
    if super::super::super::objects::unit_provider(graph, unit)? != authority.provider() {
        return Err(Error::Unit {
            unit,
            problem: MirTypeBridgeUnitProblemV1::WrongProvider,
        });
    }
    let initializer = role(
        authority,
        source,
        unit,
        InitializationCallableRole::Initializer,
        graph,
        types,
        meter,
    )?;
    let ensure = role(
        authority,
        source,
        unit,
        InitializationCallableRole::Ensure,
        graph,
        types,
        meter,
    )?;
    let signature = source
        .initialization_signature(unit, InitializationCallableRole::Ensure)
        .map_err(Error::Source)?;
    if source
        .initialization_signature(unit, InitializationCallableRole::Initializer)
        .map_err(Error::Source)?
        != signature
    {
        return Err(Error::Unit {
            unit,
            problem: MirTypeBridgeUnitProblemV1::Signature,
        });
    }
    let proof = match authority {
        MirTypeBridgeLocalAuthorityV1::Reader { .. } => {
            MirInitializationUnitProofKindV1::ReaderSemanticReplay
        }
        MirTypeBridgeLocalAuthorityV1::Producer { input, .. } => {
            meter.charge_work(
                input.materialization().initialization_roots().len() as u64,
                &WirePath::root(),
            )?;
            let root = input
                .materialization()
                .initialization_roots()
                .iter()
                .find(|root| root.identity() == unit)
                .ok_or(Error::Unit {
                    unit,
                    problem: MirTypeBridgeUnitProblemV1::ProducerInventory,
                })?;
            if root.initializer().implementation() != initializer.callable_owner()
                || root.ensure().implementation() != ensure.callable_owner()
            {
                return Err(Error::Unit {
                    unit,
                    problem: MirTypeBridgeUnitProblemV1::ProducerRole,
                });
            }
            MirInitializationUnitProofKindV1::ProducerEmitted(*root)
        }
    };
    Ok(MirTypeBridgeInitializationUnitV1 {
        unit,
        initializer,
        ensure,
        signature: signature.clone(),
        proof,
    })
}

fn role<E>(
    authority: MirTypeBridgeLocalAuthorityV1<'_>,
    source: &impl MirTypeBridgeSectionSourceAuthorityV1<E>,
    unit: PersistentInitializationUnitId,
    role: InitializationCallableRole,
    graph: &ValidatedIdentityGraph,
    types: &dyn MirTypeBridgeTypeLookupV1,
    meter: &mut BudgetMeter,
) -> Result<StrongCallableDefinitionOwner, MirTypeBridgeSectionError<E>> {
    use MirTypeBridgeSectionError as Error;
    let key = GeneratedCallableKey::Initialization { unit, role };
    meter.charge_sha256(
        PersistentGeneratedCallableId::hash_stream_length(&key)?,
        &WirePath::root(),
    )?;
    let id = PersistentGeneratedCallableId::from_key(&key)?;
    if graph.canonical_key::<_, GeneratedCallableKey>(id)?.as_ref() != &key {
        return Err(Error::Unit {
            unit,
            problem: MirTypeBridgeUnitProblemV1::MissingRole,
        });
    }
    let target = StrongCallableDefinitionOwner::GeneratedCallable(id);
    let expected = source
        .initialization_signature(unit, role)
        .map_err(Error::Source)?;
    let signature = expected.exact();
    if expected.gc_effect() != crate::GcEffect::Managed
        || signature.effect() != scoop_identity::Effect::Ordinary
        || signature.receiver().is_present()
        || !signature.parameters().is_empty()
        || !types.get(signature.result()).is_some_and(|ty| {
            matches!(
                ty.representation(),
                MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit)
            )
        })
    {
        return Err(Error::Unit {
            unit,
            problem: MirTypeBridgeUnitProblemV1::Signature,
        });
    }
    let subject = crate::CallableSignatureSubject::strong(target.callable_owner());
    let signatures = authority.foundation().as_canonical().callable_signatures();
    meter.charge_work(signatures.len() as u64, &WirePath::root())?;
    let actual = signatures
        .binary_search_by(|record| record.subject().compare_sort_key(subject))
        .ok()
        .map(|index| signatures[index].signature());
    if actual != Some(signature) {
        return Err(Error::Unit {
            unit,
            problem: MirTypeBridgeUnitProblemV1::Signature,
        });
    }
    Ok(target)
}

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    input: &mir::SingleConeStrongMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    declaration: Declaration,
    source: SourceContract,
    expected: ExactCallableSignature,
    role: mir::MirCallableLoweringRoleV1,
    meter: &mut BudgetMeter,
) -> Result<mir::ParamFreeMirCallableBindingV1, Error> {
    let implementation = declaration.implementation();
    let roots = input.materialization().callable_roots();
    let signatures = &input.module().meta.callable_signatures;
    if matches!(role, mir::MirCallableLoweringRoleV1::PureVirtualTrap { .. }) {
        work(
            (types.record_count() as u64)
                .saturating_mul(u64::from(types.record_count().checked_ilog2().unwrap_or(0)) + 1),
            meter,
        )?;
    }
    work(
        (signatures.len() as u64)
            .saturating_add(u64::from(roots.len().checked_ilog2().unwrap_or(0)) + 1)
            .saturating_add(
                (expected.parameters().len() as u64 * 2 + 8)
                    * (u64::from(types.record_count().checked_ilog2().unwrap_or(0)) + 1)
                    + 32,
            ),
        meter,
    )?;
    let index = roots
        .binary_search_by_key(&implementation.callable_owner(), |root| {
            root.implementation()
        })
        .map_err(|_| Error::MissingMirMaterialization(declaration))?;
    let root = roots[index];
    let actual = signatures
        .get(mir::CallableSignatureSubject::Strong(root.implementation()))
        .ok_or(Error::MissingSignature(declaration))?;
    if actual.signature() != &expected || expected.effect() != source.execution {
        return Err(Error::SourceSignatureMismatch(declaration));
    }
    let origin = match declaration {
        Declaration::Function(id) => mir::MirCallableOriginV1::Function(id),
        Declaration::PropertyAccessor(id) => mir::MirCallableOriginV1::Accessor(id),
    };
    Ok(mir::ParamFreeMirCallableBindingV1::try_new(
        mir::MirCallableBridgeAuthority {
            identities,
            foundation: input.foundation(),
            types,
        },
        origin,
        implementation,
        mir::MirBridgeCallableSignatureV1::new(expected.clone(), source.gc),
        mir::MirBridgeCallableSignatureV1::new(
            expected,
            input.module().functions[root.function()].gc_effect,
        ),
        role,
    )?)
}

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn project(
    input: &mir::ConeMirInput,
    identities: &ValidatedIdentityGraph,
    types: &dyn mir::MirTypeBridgeTypeLookupV1,
    declaration: Declaration,
    source: SourceContract,
    expected: ExactCallableSignature,
    role: mir::MirCallableLoweringRoleV1,
) -> Result<mir::ParamFreeMirCallableBindingV1, Error> {
    let implementation = declaration.implementation();
    let roots = input.materialization().callable_roots();
    let signatures = &input.module().meta.callable_signatures;

    let index = roots
        .binary_search_by(|root| {
            root.subject()
                .compare_sort_key(mir::CallableSignatureSubject::Strong(
                    implementation.callable_owner(),
                ))
        })
        .map_err(|_| Error::MissingMirMaterialization(declaration))?;
    let root = roots[index];
    let actual = signatures
        .get(root.subject())
        .ok_or(Error::MissingSignature(declaration))?;
    let semantic = input
        .module()
        .meta
        .source_callable_materializations
        .get(root.function())
        .ok_or(Error::MissingMirMaterialization(declaration))?;
    if semantic.signature_record().signature() != &expected || expected.effect() != source.execution
    {
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
            actual.signature().clone(),
            input.module().functions[root.function()].gc_effect,
        ),
        role,
    )?)
}

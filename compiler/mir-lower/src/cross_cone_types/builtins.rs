//! Language builtins retain type exports without inventing a source declaration arena.

use scoop_identity::CoreBuiltinNominal;

use super::*;

pub(super) fn project(
    hir: &hir::CrossConeTypeSemanticsProductionV1,
    input: &mir::SingleConeStrongMirInput,
    identities: &ValidatedIdentityGraph,
    identity: &mir::SourceExactTypeIdentity,
    nominal: PersistentTypeId,
    meter: &mut BudgetMeter,
) -> Result<Option<mir::ParamFreeMirTypeExportV1>, SourceMirTypeProductionError> {
    let (builtin, representation) = match identity.ty() {
        mir::Type::Unit => (
            CoreBuiltinNominal::Unit,
            mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::Unit),
        ),
        mir::Type::Any => (
            CoreBuiltinNominal::Any,
            mir::MirTypeRepresentationV1::Class {
                kind: mir::MirClassKindV1::Abstract,
                declared_fields: vec![],
            },
        ),
        _ => return Ok(None),
    };
    work(1, meter)?;
    let provider = builtin.declaration_key().origin();
    if nominal != builtin.identity_record().id()
        || identity.owner() != mir::SourceExactTypeOwner::Cone(provider)
    {
        return Err(SourceMirTypeProductionError::RepresentationMismatch(
            nominal,
        ));
    }
    if provider != input.module().cone {
        return Ok(None);
    }
    let exact = identity.identity_record().id();
    let source = hir.section().exact_facts();
    work(
        source.records().len().checked_ilog2().unwrap_or(0) as usize + 1,
        meter,
    )?;
    let source = source
        .get(exact)
        .ok_or(SourceMirTypeProductionError::MissingFacts(exact))?;
    Ok(Some(mir::ParamFreeMirTypeExportV1::try_new(
        mir::MirTypeBridgeAuthority {
            identities,
            foundation: input.foundation(),
        },
        exact,
        mir::MirTypeOriginV1::SourceNominal(nominal),
        facts(source)?,
        representation,
        mir::MirBaseAndInterfacesV1 {
            base: mir::MirBaseClassV1::None,
            interfaces: vec![],
        },
    )?))
}

//! Source demands checked against the actual local MIR materializations.

use scoop_identity::{ExactTypeKey, GeneratedNominalKey};

use super::*;

pub(super) fn validate(
    input: CoreShapeSupportSourceInput,
    branch: &CoreMirBridgeBranchV1,
    module: &Module,
    shapes: &[StrongSourceNominalShapeRoot],
) -> Result<Vec<SourceDeclarationKey>, SingleConeStrongMirInputError> {
    use SingleConeStrongMirInputError as Error;

    let sources = match (input, branch) {
        (CoreShapeSupportSourceInput::NotCore, CoreMirBridgeBranchV1::NotCore) => {
            return Ok(Vec::new());
        }
        (CoreShapeSupportSourceInput::Core(sources), CoreMirBridgeBranchV1::Core(_)) => sources,
        _ => return Err(Error::CoreShapeSupportSourceBranchMismatch),
    };
    let mut previous = None;
    for (index, declaration) in sources.iter().enumerate() {
        if declaration.origin() != module.cone
            || !declaration.declaration_kind().is_nominal()
            || declaration.duplicate_signature().type_parameter_count() != 0
        {
            return Err(Error::InvalidCoreShapeSupportSource { index });
        }
        let source = PersistentTypeId::from_source_declaration(declaration)
            .map_err(|error| Error::CoreShapeSupportSourceIdentity { index, error })?;
        if let Some(previous) = previous
            && previous >= source
        {
            return Err(Error::NonCanonicalCoreShapeSupportSource {
                index,
                previous,
                current: source,
            });
        }
        previous = Some(source);
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source))
            .map_err(|error| Error::CoreShapeSupportExactIdentity { index, error })?;
        let shape = shapes
            .iter()
            .find(|shape| shape.source() == source && shape.exact() == exact)
            .ok_or(Error::MissingCoreShapeSupportSource { source, exact })?;
        if !module
            .meta
            .coroutine_steps
            .iter()
            .any(|(_, step)| step.identity().result_record().id() == exact)
        {
            return Err(Error::MissingCoreCoroutineStep(exact));
        }
        if !module
            .meta
            .coroutine_slots
            .iter()
            .any(|(_, slot)| slot.identity().value_record().id() == exact)
        {
            return Err(Error::MissingCoreCoroutineSlot(exact));
        }
        if matches!(shape.ty(), Type::Unit | Type::Integer(_) | Type::Boolean | Type::Struct(_) | Type::Enum(_, _))
            && !module.meta.boxed_types.iter().any(|boxed| {
                matches!(boxed.identity().generated_type_record().key(), GeneratedNominalKey::BoxedValue { payload } if *payload == exact)
            })
        {
            return Err(Error::MissingCoreBoxedValue(exact));
        }
    }
    Ok(sources)
}

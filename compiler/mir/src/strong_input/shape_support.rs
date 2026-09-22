//! Source demands checked against the actual local MIR materializations.

use scoop_identity::{ExactTypeKey, GeneratedNominalKey};

use super::*;

/// A source demand inseparably bound to its actual nominal materialization.
#[derive(Clone, Debug, PartialEq)]
pub struct StrongSourceShapeSupportRoot {
    declaration: SourceDeclarationKey,
    shape: StrongSourceNominalShapeRoot,
}

impl StrongSourceShapeSupportRoot {
    pub const fn declaration(&self) -> &SourceDeclarationKey {
        &self.declaration
    }
    pub const fn shape(&self) -> &StrongSourceNominalShapeRoot {
        &self.shape
    }
}

pub(super) fn validate(
    sources: Vec<SourceDeclarationKey>,
    module: &Module,
    shapes: &[StrongSourceNominalShapeRoot],
) -> Result<Vec<StrongSourceShapeSupportRoot>, SingleConeStrongMirInputError> {
    use SingleConeStrongMirInputError as Error;

    let mut previous = None;
    let mut roots = Vec::with_capacity(sources.len());
    for (index, declaration) in sources.into_iter().enumerate() {
        if declaration.origin() != module.cone
            || !declaration.declaration_kind().is_nominal()
            || declaration.duplicate_signature().type_parameter_count() != 0
        {
            return Err(Error::InvalidShapeSupportSource { index });
        }
        let source = PersistentTypeId::from_source_declaration(&declaration)
            .map_err(|error| Error::ShapeSupportSourceIdentity { index, error })?;
        if let Some(previous) = previous
            && previous >= source
        {
            return Err(Error::NonCanonicalShapeSupportSource {
                index,
                previous,
                current: source,
            });
        }
        previous = Some(source);
        let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source))
            .map_err(|error| Error::ShapeSupportExactIdentity { index, error })?;
        let shape = shapes
            .iter()
            .find(|shape| shape.source() == source && shape.exact() == exact)
            .ok_or(Error::MissingShapeSupportSource { source, exact })?;
        if !module
            .meta
            .coroutine_steps
            .iter()
            .any(|(_, step)| step.identity().result_record().id() == exact)
        {
            return Err(Error::MissingCoroutineStep(exact));
        }
        if !module
            .meta
            .coroutine_slots
            .iter()
            .any(|(_, slot)| slot.identity().value_record().id() == exact)
        {
            return Err(Error::MissingCoroutineSlot(exact));
        }
        if matches!(shape.ty(), Type::Unit | Type::Integer(_) | Type::Boolean | Type::Struct(_) | Type::Enum(_, _))
            && !module.meta.boxed_types.iter().any(|boxed| {
                matches!(boxed.identity().generated_type_record().key(), GeneratedNominalKey::BoxedValue { payload } if *payload == exact)
            })
        {
            return Err(Error::MissingBoxedValue(exact));
        }
        roots.push(StrongSourceShapeSupportRoot {
            declaration,
            shape: shape.clone(),
        });
    }
    Ok(roots)
}

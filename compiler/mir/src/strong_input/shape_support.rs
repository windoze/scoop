//! Source demands checked against the actual local MIR materializations.

use scoop_identity::{ExactTypeKey, GeneratedNominalKey};

use super::*;

/// A source demand inseparably bound to its actual nominal materialization.
#[derive(Clone, Debug, PartialEq)]
pub struct StrongSourceShapeSupportRoot {
    declaration: SourceDeclarationKey,
    shape: StrongSourceNominalShapeRoot,
    boxed: StrongBoxedShapeSupportRoot,
    coroutine_step: StrongGeneratedNominalShapeRoot,
    coroutine_slot: StrongGeneratedNominalShapeRoot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongBoxedShapeSupportRoot {
    Available(StrongGeneratedNominalShapeRoot),
    ReferenceNominalRequiresNoBox,
}

impl StrongSourceShapeSupportRoot {
    pub const fn declaration(&self) -> &SourceDeclarationKey {
        &self.declaration
    }
    pub const fn shape(&self) -> &StrongSourceNominalShapeRoot {
        &self.shape
    }
    pub const fn boxed(&self) -> StrongBoxedShapeSupportRoot {
        self.boxed
    }
    pub const fn coroutine_step(&self) -> StrongGeneratedNominalShapeRoot {
        self.coroutine_step
    }
    pub const fn coroutine_slot(&self) -> StrongGeneratedNominalShapeRoot {
        self.coroutine_slot
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
        let step = module
            .meta
            .coroutine_steps
            .iter()
            .find(|(_, step)| step.identity().result_record().id() == exact)
            .and_then(|(_, step)| {
                bind(
                    module,
                    GeneratedExactTypeLocation::Enum(step.enum_id()),
                    step.identity().generated_type_record().key(),
                )
            })
            .ok_or(Error::MissingCoroutineStep(exact))?;
        let slot = module
            .meta
            .coroutine_slots
            .iter()
            .find(|(_, slot)| slot.identity().value_record().id() == exact)
            .and_then(|(_, slot)| {
                bind(
                    module,
                    GeneratedExactTypeLocation::Enum(slot.enum_id()),
                    slot.identity().generated_type_record().key(),
                )
            })
            .ok_or(Error::MissingCoroutineSlot(exact))?;
        let boxed = if matches!(
            shape.ty(),
            Type::Unit | Type::Integer(_) | Type::Boolean | Type::Struct(_) | Type::Enum(_, _)
        ) {
            let boxed = module.meta.boxed_types.iter().find(|boxed| {
                matches!(boxed.identity().generated_type_record().key(), GeneratedNominalKey::BoxedValue { payload } if *payload == exact)
            })
                .and_then(|boxed| bind(module, GeneratedExactTypeLocation::Class(boxed.class()), boxed.identity().generated_type_record().key()))
                .ok_or(Error::MissingBoxedValue(exact))?;
            StrongBoxedShapeSupportRoot::Available(boxed)
        } else {
            StrongBoxedShapeSupportRoot::ReferenceNominalRequiresNoBox
        };
        roots.push(StrongSourceShapeSupportRoot {
            declaration,
            shape: shape.clone(),
            boxed,
            coroutine_step: step,
            coroutine_slot: slot,
        });
    }
    Ok(roots)
}

fn bind(
    module: &Module,
    location: GeneratedExactTypeLocation,
    role: &GeneratedNominalKey,
) -> Option<StrongGeneratedNominalShapeRoot> {
    let identity = module.meta.generated_exact_types.get(location)?;
    (identity.nominal_record().key() == role).then_some(StrongGeneratedNominalShapeRoot {
        location,
        nominal: identity.nominal_record().id(),
        exact: identity.exact_record().id(),
    })
}

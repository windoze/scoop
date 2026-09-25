use super::*;
use crate::GeneratedExactTypeOwner;
use scoop_identity::GeneratedNominalKey;

#[cfg(test)]
mod tests;

/// A finite helper owned by a dependency, never a local Strong definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongDependencyGeneratedNominalShapeRoot {
    shape: StrongGeneratedNominalShapeRoot,
    provider: ConeIdentity,
    source: PersistentTypeId,
    source_exact: PersistentExactTypeId,
}

impl StrongDependencyGeneratedNominalShapeRoot {
    pub const fn location(self) -> GeneratedExactTypeLocation {
        self.shape.location()
    }

    pub const fn nominal(self) -> PersistentTypeId {
        self.shape.nominal()
    }

    pub const fn exact(self) -> PersistentExactTypeId {
        self.shape.exact()
    }

    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }

    pub const fn source(self) -> PersistentTypeId {
        self.source
    }

    pub const fn source_exact(self) -> PersistentExactTypeId {
        self.source_exact
    }
}

pub(super) struct Partition {
    pub local: Vec<StrongGeneratedNominalShapeRoot>,
    pub dependencies: Vec<StrongDependencyGeneratedNominalShapeRoot>,
}

pub(super) fn partition(module: &Module) -> Result<Partition, SingleConeStrongMirInputError> {
    use SingleConeStrongMirInputError as Error;
    let mut local = Vec::new();
    let mut dependencies = Vec::new();
    for identity in module.meta.generated_exact_types.iter() {
        let location = identity.location();
        if identity.owner() != &GeneratedExactTypeOwner::ConeOwned {
            return Err(Error::OdrGeneratedNominalShape(location));
        }
        let shape = StrongGeneratedNominalShapeRoot {
            location,
            nominal: identity.nominal_record().id(),
            exact: identity.exact_record().id(),
        };
        let source_exact = match identity.nominal_record().key() {
            GeneratedNominalKey::BoxedValue { payload } => *payload,
            GeneratedNominalKey::CoroutineStep { result } => *result,
            GeneratedNominalKey::CoroutineSlot { value } => *value,
            GeneratedNominalKey::ClosureEnvironment { .. }
            | GeneratedNominalKey::CallableAdapterEnvironment { .. }
            | GeneratedNominalKey::ContinuationAdapterEnvironment { .. }
            | GeneratedNominalKey::CoroutineFrame { .. }
            | GeneratedNominalKey::ObjectBackingClass { .. } => {
                local.push(shape);
                continue;
            }
        };
        let (source, provider) =
            source_owner(&module.meta.source_exact_types, location, source_exact)?;
        if provider == module.cone {
            local.push(shape);
        } else {
            if let GeneratedExactTypeLocation::Class(class) = location
                && module
                    .meta
                    .boxing_adjusts
                    .iter()
                    .any(|adjust| adjust.boxed() == class)
            {
                return Err(Error::ForeignGeneratedHelperCallable(location));
            }
            dependencies.push(StrongDependencyGeneratedNominalShapeRoot {
                shape,
                provider,
                source,
                source_exact,
            });
        }
    }
    local.sort_unstable_by_key(|root| root.exact());
    dependencies.sort_unstable_by_key(|root| (root.provider(), root.exact()));
    Ok(Partition {
        local,
        dependencies,
    })
}

fn source_owner(
    sources: &crate::SourceExactTypeIdentities,
    location: GeneratedExactTypeLocation,
    exact: PersistentExactTypeId,
) -> Result<(PersistentTypeId, ConeIdentity), SingleConeStrongMirInputError> {
    use SingleConeStrongMirInputError as Error;
    let source = sources
        .get_by_identity(exact)
        .ok_or(Error::MissingGeneratedSourceExact { location, exact })?;
    match (source.identity_record().key(), source.owner()) {
        (ExactTypeKey::Nominal(source), SourceExactTypeOwner::Cone(provider)) => {
            Ok((*source, provider))
        }
        _ => Err(Error::InvalidGeneratedSourceOwner { location, exact }),
    }
}

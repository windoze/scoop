//! Object values and their actual initialization callable implementations.

use super::*;
use crate::{SingleConeStrongMirInput, StrongInitializationUnitMaterializationRoot};

mod callables;
mod sources;

pub struct MirObjectValueProductionV1 {
    callables: CanonicalMirCallableBindingsV1,
    objects: CanonicalMirObjectValuesV1,
}

impl MirObjectValueProductionV1 {
    /// Selects only local object type exports and binds their real singleton
    /// roots. Dependency records remain borrowed through the shared index.
    pub fn from_strong_input(
        input: &SingleConeStrongMirInput,
        local_types: &CanonicalParamFreeMirTypeExportsV1,
        identities: &ValidatedIdentityGraph,
        types: &dyn MirTypeBridgeTypeLookupV1,
    ) -> Result<Self, MirObjectProductionError> {
        let sources = sources::project(input, local_types)?;
        let callables = callables::project(input, &sources, identities, types)?;
        let authority = MirObjectBridgeAuthority {
            identities,
            types,
            callables: &callables,
        };
        let mut objects = Vec::new();
        reserve(&mut objects, sources.len())?;
        for source in sources {
            objects.push(ParamFreeMirObjectValueV1::try_new(
                authority,
                source.value,
                source.backing,
                source.unit.identity(),
                callables::implementation(source.unit.ensure())?,
                MirObjectValueReadPlanV1::PublishedSingletonRoot {
                    object: source.exact,
                },
            )?);
        }
        Ok(Self {
            callables,
            objects: CanonicalMirObjectValuesV1::try_new(objects)?,
        })
    }

    pub const fn callables(&self) -> &CanonicalMirCallableBindingsV1 {
        &self.callables
    }
    pub const fn objects(&self) -> &CanonicalMirObjectValuesV1 {
        &self.objects
    }
    pub fn into_parts(self) -> (CanonicalMirCallableBindingsV1, CanonicalMirObjectValuesV1) {
        (self.callables, self.objects)
    }
}

struct ObjectSource {
    value: PersistentObjectValueId,
    exact: PersistentExactTypeId,
    backing: PersistentExactTypeId,
    unit: StrongInitializationUnitMaterializationRoot,
}

#[derive(Debug)]
pub enum MirObjectProductionError {
    Resource(WireError),
    Object(MirObjectBridgeError),
    Callable(MirCallableBridgeError),
    MissingSingletonType {
        value: PersistentObjectValueId,
    },
    SourceRepresentation {
        exact: PersistentExactTypeId,
    },
    IncompleteObjects {
        expected: usize,
        actual: usize,
    },
    InitializationRole {
        implementation: crate::CallableOwner,
    },
    MissingSignature {
        implementation: crate::CallableOwner,
    },
}
impl From<WireError> for MirObjectProductionError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl From<MirObjectBridgeError> for MirObjectProductionError {
    fn from(error: MirObjectBridgeError) -> Self {
        Self::Object(error)
    }
}
impl From<MirCallableBridgeError> for MirObjectProductionError {
    fn from(error: MirCallableBridgeError) -> Self {
        Self::Callable(error)
    }
}
impl std::fmt::Display for MirObjectProductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot produce MIR object values: {self:?}")
    }
}
impl std::error::Error for MirObjectProductionError {}

fn reserve<T>(records: &mut Vec<T>, count: usize) -> Result<(), WireError> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(records, count, &path)
}

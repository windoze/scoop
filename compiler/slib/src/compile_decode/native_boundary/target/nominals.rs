//! Private representation inputs shared by native and ordinary Scoop ABI replay.

use std::borrow::Cow;

use scoop_hir::NativeBoundaryCAbiV1;

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct AbiNominalDefinition<'a> {
    shape: Cow<'a, NativeBoundaryNominalShape>,
    type_parameter_count: u32,
    c_abi: NativeBoundaryCAbiV1,
    is_interface: bool,
}

impl<'a> AbiNominalDefinition<'a> {
    pub(super) fn native(
        record: &'a NativeBoundaryTypeDefinitionRecord,
        is_interface: bool,
    ) -> Self {
        Self {
            shape: Cow::Borrowed(record.shape()),
            type_parameter_count: record.type_parameter_count(),
            c_abi: record.c_abi(),
            is_interface,
        }
    }

    pub(super) fn shared(
        shape: NativeBoundaryNominalShape,
        type_parameter_count: u32,
        is_interface: bool,
    ) -> Self {
        Self {
            shape: Cow::Owned(shape),
            type_parameter_count,
            c_abi: NativeBoundaryCAbiV1::SourceRepresentation,
            is_interface,
        }
    }

    pub(super) fn shape(&self) -> &NativeBoundaryNominalShape {
        &self.shape
    }

    pub(super) const fn is_interface(&self) -> bool {
        self.is_interface
    }

    pub(super) const fn c_abi(&self) -> NativeBoundaryCAbiV1 {
        self.c_abi
    }

    pub(super) fn agrees_with_source(&self, other: &Self) -> bool {
        self.type_parameter_count == other.type_parameter_count
            && self.shape == other.shape
            && self.is_interface == other.is_interface
    }
}

pub(super) fn native_definitions<'a>(
    records: &'a [NativeBoundaryTypeDefinitionRecord],
    identities: &scoop_identity::ValidatedIdentityGraph,
) -> Result<HashMap<NativeBoundaryNominalOwner, AbiNominalDefinition<'a>>, NativeBoundaryCompileError>
{
    let mut definitions = HashMap::new();
    let path = WirePath::root().field(34);

    scoop_wire::allocation::try_reserve_map(&mut definitions, records.len(), &path)
        .map_err(NativeBoundaryCompileError::Resource)?;
    for record in records {
        if definitions
            .insert(
                record.owner(),
                AbiNominalDefinition::native(record, is_interface(record.owner(), identities)?),
            )
            .is_some()
        {
            return Err(NativeBoundaryCompileError::ConflictingTypeWitness {
                owner: record.owner(),
            });
        }
    }
    Ok(definitions)
}

pub(super) fn is_interface(
    owner: NativeBoundaryNominalOwner,
    identities: &scoop_identity::ValidatedIdentityGraph,
) -> Result<bool, NativeBoundaryCompileError> {
    let key = match owner {
        NativeBoundaryNominalOwner::Concrete(id) => {
            identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)
        }
        NativeBoundaryNominalOwner::GenericTemplate(id) => {
            identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)
        }
    }
    .map_err(NativeBoundaryCompileError::Reference)?;
    Ok(key.declaration_kind() == scoop_identity::SourceDeclarationKind::Interface)
}

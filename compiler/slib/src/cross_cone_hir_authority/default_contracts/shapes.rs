use std::collections::BTreeMap;

use scoop_hir::{
    CrossConeHirInterfaceSectionV1, NominalInterfaceShapeAuthority, PublicNominalKindV1,
    PublicNominalShapeV1, SourceNominalId,
};
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, CoreBuiltinNominal, IdentityReferenceError,
    PersistentGenericTypeId, PersistentTypeId, ValidatedIdentityGraph,
};
use scoop_wire::WireError;

/// A transient type-shape index over already checked metadata. It grants no
/// lookup or access rights to the private declarations used for source typing.
pub(super) struct DefaultNominalShapes<'g> {
    shapes: BTreeMap<SourceNominalId, PublicNominalShapeV1>,
    pub(super) identities: &'g ValidatedIdentityGraph,
}

impl<'g> DefaultNominalShapes<'g> {
    pub(super) fn new<'a>(
        identities: &'g ValidatedIdentityGraph,
        providers: impl IntoIterator<Item = (ConeIdentity, &'a CrossConeHirInterfaceSectionV1)>,
    ) -> Result<Self, DefaultMetadataNominalError> {
        let mut result = Self {
            shapes: BTreeMap::new(),
            identities,
        };
        for (provider, interface) in providers {
            for record in interface.nominal_interfaces().all_records() {
                result.insert(
                    record.declaration(),
                    PublicNominalShapeV1::new(record.kind(), record.type_parameters().len_u32()),
                )?;
            }
            for builtin in [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any] {
                let record = builtin.identity_record();
                if provider == record.key().origin() {
                    let kind = match builtin {
                        CoreBuiltinNominal::Unit => PublicNominalKindV1::Struct,
                        CoreBuiltinNominal::Any => PublicNominalKindV1::Class,
                    };
                    result
                        .shapes
                        .entry(SourceNominalId::Concrete(record.id()))
                        .or_insert(PublicNominalShapeV1::new(kind, 0));
                }
            }
        }
        Ok(result)
    }

    fn insert(
        &mut self,
        declaration: SourceNominalId,
        shape: PublicNominalShapeV1,
    ) -> Result<(), DefaultMetadataNominalError> {
        if self.shapes.insert(declaration, shape).is_some() {
            return Err(DefaultMetadataNominalError::Duplicate(declaration));
        }
        Ok(())
    }

    pub(super) fn get(
        &self,
        declaration: SourceNominalId,
    ) -> Result<PublicNominalShapeV1, DefaultMetadataNominalError> {
        self.shapes
            .get(&declaration)
            .copied()
            .ok_or(DefaultMetadataNominalError::Missing(declaration))
    }
}

impl NominalInterfaceShapeAuthority<DefaultMetadataNominalError> for DefaultNominalShapes<'_> {
    fn concrete_nominal_shape(
        &mut self,
        declaration: PersistentTypeId,
    ) -> Result<PublicNominalShapeV1, DefaultMetadataNominalError> {
        self.get(SourceNominalId::Concrete(declaration))
    }
    fn generic_nominal_shape(
        &mut self,
        declaration: PersistentGenericTypeId,
    ) -> Result<PublicNominalShapeV1, DefaultMetadataNominalError> {
        self.get(SourceNominalId::GenericTemplate(declaration))
    }
}

#[derive(Debug)]
pub enum DefaultMetadataNominalError {
    Resource(WireError),
    Missing(SourceNominalId),
    Duplicate(SourceNominalId),
    Identity(IdentityReferenceError),
    NonFunctionSignature(CallableTemplateOrigin),
}
impl From<WireError> for DefaultMetadataNominalError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for DefaultMetadataNominalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Missing(declaration) => write!(
                f,
                "default source type {declaration:?} has no reachable declaration"
            ),
            Self::Duplicate(declaration) => write!(
                f,
                "default source type {declaration:?} occurs in multiple provider tables"
            ),
            Self::Identity(error) => error.fmt(f),
            Self::NonFunctionSignature(declaration) => write!(
                f,
                "default local signature target {declaration:?} is not a function"
            ),
        }
    }
}
impl std::error::Error for DefaultMetadataNominalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Identity(error) => Some(error),
            Self::Missing(_) | Self::Duplicate(_) | Self::NonFunctionSignature(_) => None,
        }
    }
}

//! Finite machine-shape requirements of the published nominal declarations.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use scoop_identity::{
    BindableEntity, ConeIdentity, CoreBuiltinNominal, ExactTypeKey, PersistentExactTypeId,
    PersistentExportBindingId, PersistentTypeId, SourceDeclarationKey,
};

use crate::{
    CanonicalDirectPublicSurfaceV1, CanonicalHirFoundation, CanonicalPublicExportBindingsV1,
    ExportBindingSourceV1, HirExportBindingIdentities,
};

mod materialization;
mod shared;

const LANGUAGE_BUILTINS: [CoreBuiltinNominal; 2] =
    [CoreBuiltinNominal::Unit, CoreBuiltinNominal::Any];

pub use materialization::{
    NominalMaterializationClosure, NominalMaterializationClosureError,
    NominalMaterializationRequirementV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicNominalShapeRequirementV1 {
    source: PersistentTypeId,
    exact: PersistentExactTypeId,
}

impl PublicNominalShapeRequirementV1 {
    pub const fn source(self) -> PersistentTypeId {
        self.source
    }

    pub const fn exact(self) -> PersistentExactTypeId {
        self.exact
    }
}

/// Local, non-generic nominal roots, including owned language builtins.
/// Aliases and reexports never add roots.
/// This projection is not serialized as a second declaration inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicNominalShapeRequirementsV1 {
    roots: Vec<PublicNominalShapeRequirementV1>,
}

impl PublicNominalShapeRequirementsV1 {
    pub fn from_public_bindings(
        producer: ConeIdentity,
        bindings: &CanonicalPublicExportBindingsV1,
        identities: &HirExportBindingIdentities,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let keys = identities
            .iter()
            .map(|record| (record.id(), record.key()))
            .collect::<BTreeMap<_, _>>();
        let mut sources = BTreeSet::new();
        for record in bindings.records() {
            let key = keys.get(&record.binding()).ok_or(
                PublicNominalShapeProjectionError::MissingBinding(record.binding()),
            )?;
            if key.exporter() == producer
                && let ExportBindingSourceV1::DeclaredCurrent {
                    declaration: BindableEntity::Type(source),
                } = record.source()
            {
                sources.insert(*source);
            }
        }
        Self::from_sources(producer, sources)
    }

    pub fn from_direct_surface(
        producer: ConeIdentity,
        surface: &CanonicalDirectPublicSurfaceV1,
        foundation: &CanonicalHirFoundation,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let mut sources = BTreeSet::new();
        for binding in surface.bindings() {
            let key = foundation
                .export_binding_key(*binding)
                .ok_or(PublicNominalShapeProjectionError::MissingBinding(*binding))?;
            if let BindableEntity::Type(source) = key.target() {
                sources.insert(source);
            }
        }
        let requirements = Self::from_sources(producer, sources)?;
        for root in requirements.roots() {
            if foundation
                .source_type_by_bytes(root.source.as_array())
                .is_none()
            {
                return Err(PublicNominalShapeProjectionError::MissingSourceNominal(
                    root.source,
                ));
            }
        }
        Ok(requirements)
    }

    fn from_sources(
        producer: ConeIdentity,
        mut sources: BTreeSet<PersistentTypeId>,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        sources.extend(LANGUAGE_BUILTINS.into_iter().filter_map(|builtin| {
            let record = builtin.identity_record();
            (record.key().origin() == producer).then_some(record.id())
        }));
        let roots = sources
            .into_iter()
            .map(|source| {
                PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(source))
                    .map(|exact| PublicNominalShapeRequirementV1 { source, exact })
                    .map_err(PublicNominalShapeProjectionError::Identity)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { roots })
    }

    pub fn roots(&self) -> &[PublicNominalShapeRequirementV1] {
        &self.roots
    }

    pub fn source_declarations(
        &self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<Vec<SourceDeclarationKey>, PublicNominalShapeProjectionError> {
        self.roots
            .iter()
            .map(|root| {
                foundation
                    .source_type_by_bytes(root.source.as_array())
                    .map(|(_, key)| key.clone())
                    .ok_or(PublicNominalShapeProjectionError::MissingSourceNominal(
                        root.source,
                    ))
            })
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublicNominalShapeProjectionError {
    MissingBinding(PersistentExportBindingId),
    MissingSourceNominal(PersistentTypeId),
    Identity(scoop_wire::HashError),
    Materialization(NominalMaterializationClosureError),
    SharedDeclarations(String),
}

impl fmt::Display for PublicNominalShapeProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot project public nominal shape obligations: {self:?}"
        )
    }
}

impl std::error::Error for PublicNominalShapeProjectionError {}

#[cfg(test)]
mod tests;

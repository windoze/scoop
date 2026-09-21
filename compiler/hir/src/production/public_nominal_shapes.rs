//! Transient shape obligations derived from the shared public binding surface.

use std::{collections::BTreeSet, fmt};

use scoop_identity::{
    BindableEntity, ExactTypeKey, PersistentExactTypeId, PersistentExportBindingId,
    PersistentTypeId, SourceDeclarationKey,
};

use crate::{
    CanonicalDirectPublicSurfaceV1, CanonicalHirFoundation, CanonicalPublicExportBindingsV1,
    ExportBindingSourceV1,
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

/// Local, non-generic nominal roots. Aliases and reexports never add roots.
/// This projection is not serialized as a second declaration inventory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicNominalShapeRequirementsV1 {
    roots: Vec<PublicNominalShapeRequirementV1>,
}

impl PublicNominalShapeRequirementsV1 {
    pub fn from_public_bindings(
        bindings: &CanonicalPublicExportBindingsV1,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        Self::from_sources(
            bindings
                .records()
                .iter()
                .filter_map(|record| match record.source() {
                    ExportBindingSourceV1::DeclaredCurrent {
                        declaration: BindableEntity::Type(source),
                    } => Some(*source),
                    _ => None,
                })
                .collect(),
        )
    }

    pub fn from_direct_surface(
        surface: &CanonicalDirectPublicSurfaceV1,
        foundation: &CanonicalHirFoundation,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
        let mut sources = BTreeSet::new();
        for binding in surface.bindings() {
            let key = foundation
                .export_binding_key(*binding)
                .ok_or(PublicNominalShapeProjectionError::MissingBinding(*binding))?;
            if let BindableEntity::Type(source) = key.target() {
                if foundation.source_type_by_bytes(source.as_array()).is_none() {
                    return Err(PublicNominalShapeProjectionError::MissingSourceNominal(
                        source,
                    ));
                }
                sources.insert(source);
            }
        }
        Self::from_sources(sources)
    }

    fn from_sources(
        sources: BTreeSet<PersistentTypeId>,
    ) -> Result<Self, PublicNominalShapeProjectionError> {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicNominalShapeProjectionError {
    MissingBinding(PersistentExportBindingId),
    MissingSourceNominal(PersistentTypeId),
    Identity(scoop_wire::HashError),
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

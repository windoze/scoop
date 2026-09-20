use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    ConeIdentity, ExactTypeKey, GeneratedNominalKey, PersistentExactTypeId,
    PersistentPropertyAccessorId, PersistentTypeId, PropertyAccessorKey, SourceDeclarationKey,
};

use crate::*;

/// Independent HIR evidence used to validate the produced M23-6 foundation
/// tables. None of these lookups read the candidate type-semantics section.
#[derive(Clone, Debug)]
pub struct CrossConeTypeSemanticsFoundationV1 {
    provider: ConeIdentity,
    exact_keys: BTreeMap<PersistentExactTypeId, ExactTypeKey>,
    sources: BTreeMap<SourceNominalId, TypeSemanticsSourceEvidenceV1>,
    representations: BTreeMap<PersistentTypeId, TypeSemanticsRepresentationEvidenceV1>,
    generated_nominals: BTreeMap<PersistentTypeId, GeneratedNominalKey>,
    accessor_keys: BTreeMap<PersistentPropertyAccessorId, PropertyAccessorKey>,
    valid_definition_sources: BTreeSet<ExportDefinitionSourceV1>,
    source_roots: Vec<SourceNominalId>,
    local_exact_facts: CanonicalPersistentIdsV1<PersistentExactTypeId>,
    dependency_facts: Vec<TypeSectionDependencyFactV1>,
    local_inheritance_edges: Vec<NominalInheritanceEdgesV1>,
    fact_shapes: BTreeMap<PersistentExactTypeId, ExactTypeFactShapeV1>,
    representation_owners: CanonicalPersistentIdsV1<PersistentTypeId>,
}

#[derive(Clone, Debug)]
pub(super) struct TypeSemanticsSourceEvidenceV1 {
    pub key: SourceDeclarationKey,
    pub access: DeclarationAccessSourceV1,
}

#[derive(Clone, Debug)]
pub(super) struct TypeSemanticsRepresentationEvidenceV1 {
    pub shape: NominalRepresentationShapeV1,
    pub public_value_shape: Option<NominalSourceShapeV1>,
    pub object_record: Option<NominalRepresentationSupportV1>,
}

impl CrossConeTypeSemanticsFoundationV1 {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        provider: ConeIdentity,
        exact_keys: BTreeMap<PersistentExactTypeId, ExactTypeKey>,
        sources: BTreeMap<SourceNominalId, TypeSemanticsSourceEvidenceV1>,
        representations: BTreeMap<PersistentTypeId, TypeSemanticsRepresentationEvidenceV1>,
        generated_nominals: BTreeMap<PersistentTypeId, GeneratedNominalKey>,
        accessor_keys: BTreeMap<PersistentPropertyAccessorId, PropertyAccessorKey>,
        valid_definition_sources: BTreeSet<ExportDefinitionSourceV1>,
        source_roots: Vec<SourceNominalId>,
        local_exact_facts: CanonicalPersistentIdsV1<PersistentExactTypeId>,
        dependency_facts: Vec<TypeSectionDependencyFactV1>,
        local_inheritance_edges: Vec<NominalInheritanceEdgesV1>,
        fact_shapes: BTreeMap<PersistentExactTypeId, ExactTypeFactShapeV1>,
        representation_owners: CanonicalPersistentIdsV1<PersistentTypeId>,
    ) -> Self {
        Self {
            provider,
            exact_keys,
            sources,
            representations,
            generated_nominals,
            accessor_keys,
            valid_definition_sources,
            source_roots,
            local_exact_facts,
            dependency_facts,
            local_inheritance_edges,
            fact_shapes,
            representation_owners,
        }
    }

    pub fn source_roots(&self) -> &[SourceNominalId] {
        &self.source_roots
    }

    pub const fn local_exact_facts(&self) -> &CanonicalPersistentIdsV1<PersistentExactTypeId> {
        &self.local_exact_facts
    }

    pub fn dependency_facts(&self) -> &[TypeSectionDependencyFactV1] {
        &self.dependency_facts
    }

    pub fn local_inheritance_edges(&self) -> &[NominalInheritanceEdgesV1] {
        &self.local_inheritance_edges
    }

    pub fn fact_shape(&self, exact: PersistentExactTypeId) -> Option<&ExactTypeFactShapeV1> {
        self.fact_shapes.get(&exact)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CrossConeTypeSemanticsFoundationError {
    UnknownExactType(PersistentExactTypeId),
    UnknownSourceNominal(SourceNominalId),
    UnknownRepresentation(PersistentTypeId),
    UnknownGeneratedNominal(PersistentTypeId),
    UnknownPropertyAccessor(PersistentPropertyAccessorId),
    UnknownDefinitionSource,
}

impl fmt::Display for CrossConeTypeSemanticsFoundationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownExactType(exact) => write!(formatter, "unknown exact type {exact}"),
            Self::UnknownSourceNominal(source) => {
                write!(formatter, "unknown source nominal {source:?}")
            }
            Self::UnknownRepresentation(owner) => {
                write!(formatter, "unknown representation owner {owner}")
            }
            Self::UnknownGeneratedNominal(owner) => {
                write!(formatter, "unknown generated nominal {owner}")
            }
            Self::UnknownPropertyAccessor(accessor) => {
                write!(formatter, "unknown property accessor {accessor}")
            }
            Self::UnknownDefinitionSource => formatter.write_str("unknown definition source"),
        }
    }
}

impl std::error::Error for CrossConeTypeSemanticsFoundationError {}

impl NominalInheritanceSemanticAuthority<CrossConeTypeSemanticsFoundationError>
    for CrossConeTypeSemanticsFoundationV1
{
    fn exact_type_key(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeKey, CrossConeTypeSemanticsFoundationError> {
        self.exact_keys
            .get(&exact)
            .ok_or(CrossConeTypeSemanticsFoundationError::UnknownExactType(
                exact,
            ))
    }

    fn nominal_declaration_key(
        &self,
        owner: SourceNominalId,
    ) -> Result<&SourceDeclarationKey, CrossConeTypeSemanticsFoundationError> {
        self.sources.get(&owner).map(|source| &source.key).ok_or(
            CrossConeTypeSemanticsFoundationError::UnknownSourceNominal(owner),
        )
    }

    fn nominal_access_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&DeclarationAccessSourceV1, CrossConeTypeSemanticsFoundationError> {
        self.sources.get(&owner).map(|source| &source.access).ok_or(
            CrossConeTypeSemanticsFoundationError::UnknownSourceNominal(owner),
        )
    }

    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, CrossConeTypeSemanticsFoundationError> {
        self.nominal_access_source(owner)
            .map(DeclarationAccessSourceV1::definition_origin)
    }

    fn validate_definition_source(
        &self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), CrossConeTypeSemanticsFoundationError> {
        self.valid_definition_sources
            .contains(source)
            .then_some(())
            .ok_or(CrossConeTypeSemanticsFoundationError::UnknownDefinitionSource)
    }

    fn object_representation(
        &self,
        owner: PersistentTypeId,
    ) -> Result<&NominalRepresentationSupportV1, CrossConeTypeSemanticsFoundationError> {
        self.representations
            .get(&owner)
            .and_then(|source| source.object_record.as_ref())
            .ok_or(CrossConeTypeSemanticsFoundationError::UnknownRepresentation(owner))
    }

    fn generated_nominal_key(
        &self,
        nominal: PersistentTypeId,
    ) -> Result<&GeneratedNominalKey, CrossConeTypeSemanticsFoundationError> {
        self.generated_nominals
            .get(&nominal)
            .ok_or(CrossConeTypeSemanticsFoundationError::UnknownGeneratedNominal(nominal))
    }
}

impl ExactTypeFactsSemanticAuthority<CrossConeTypeSemanticsFoundationError>
    for CrossConeTypeSemanticsFoundationV1
{
    fn fact_shape(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeFactShapeV1, CrossConeTypeSemanticsFoundationError> {
        self.fact_shapes
            .get(&exact)
            .ok_or(CrossConeTypeSemanticsFoundationError::UnknownExactType(
                exact,
            ))
    }
}

impl NominalRepresentationSemanticAuthority<CrossConeTypeSemanticsFoundationError>
    for CrossConeTypeSemanticsFoundationV1
{
    fn current_provider(&self) -> ConeIdentity {
        self.provider
    }

    fn required_representation_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentTypeId>, CrossConeTypeSemanticsFoundationError>
    {
        Ok(&self.representation_owners)
    }

    fn representation_source(
        &self,
        owner: PersistentTypeId,
    ) -> Result<NominalRepresentationSourceV1<'_>, CrossConeTypeSemanticsFoundationError> {
        let representation = self
            .representations
            .get(&owner)
            .ok_or(CrossConeTypeSemanticsFoundationError::UnknownRepresentation(owner))?;
        let source = self.sources.get(&SourceNominalId::Concrete(owner)).ok_or(
            CrossConeTypeSemanticsFoundationError::UnknownSourceNominal(SourceNominalId::Concrete(
                owner,
            )),
        )?;
        let public_value_shape = match representation.public_value_shape.as_ref() {
            Some(shape) => NominalRepresentationPublicValueShapeV1::PublicValueShape(shape),
            None => NominalRepresentationPublicValueShapeV1::NoPublicValueShape,
        };
        Ok(NominalRepresentationSourceV1 {
            key: &source.key,
            access: &source.access,
            shape: &representation.shape,
            public_value_shape,
        })
    }
}

impl TypeSectionFoundationSemanticAuthority<CrossConeTypeSemanticsFoundationError>
    for CrossConeTypeSemanticsFoundationV1
{
    fn local_exact_facts(
        &self,
    ) -> Result<
        &CanonicalPersistentIdsV1<PersistentExactTypeId>,
        CrossConeTypeSemanticsFoundationError,
    > {
        Ok(&self.local_exact_facts)
    }

    fn dependency_facts(
        &self,
    ) -> Result<&[TypeSectionDependencyFactV1], CrossConeTypeSemanticsFoundationError> {
        Ok(&self.dependency_facts)
    }

    fn local_source_roots(
        &self,
    ) -> Result<&[SourceNominalId], CrossConeTypeSemanticsFoundationError> {
        Ok(&self.source_roots)
    }

    fn local_inheritance_edges(
        &self,
    ) -> Result<&[NominalInheritanceEdgesV1], CrossConeTypeSemanticsFoundationError> {
        Ok(&self.local_inheritance_edges)
    }

    fn selected_accessor_key(
        &self,
        accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, CrossConeTypeSemanticsFoundationError> {
        self.accessor_keys
            .get(&accessor)
            .ok_or(CrossConeTypeSemanticsFoundationError::UnknownPropertyAccessor(accessor))
    }
}

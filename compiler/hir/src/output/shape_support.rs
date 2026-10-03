use std::fmt;

use crate::{LocalConcreteHir, PublicNominalShapeRequirementsV1, concrete};
use scoop_identity::{
    CoreBuiltinNominal, ExactTypeKey, PersistentExactTypeId, PersistentTypeId,
    SourceDeclarationKey, SourceDeclarationKind,
};

/// One local shape-support requirement translated into the LocalConcrete HIR
/// type-id domain without discarding its persistent HIR identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalShapeSupportRoot {
    declaration: SourceDeclarationKey,
    source: PersistentTypeId,
    exact: PersistentExactTypeId,
    ty: concrete::TypeId,
    boxed_value: LocalBoxedValueRequirement,
}

impl LocalShapeSupportRoot {
    pub const fn declaration(&self) -> &SourceDeclarationKey {
        &self.declaration
    }

    pub const fn source(&self) -> PersistentTypeId {
        self.source
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }

    pub const fn ty(&self) -> concrete::TypeId {
        self.ty
    }

    pub const fn boxed_value(&self) -> LocalBoxedValueRequirement {
        self.boxed_value
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalBoxedValueRequirement {
    Required,
    NotApplicable,
}

/// Complete local projection of the current Cone shape-support demands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalShapeSupportPlan {
    roots: Vec<LocalShapeSupportRoot>,
}

impl LocalShapeSupportPlan {
    pub fn try_new(
        module: &LocalConcreteHir,
        requirements: &PublicNominalShapeRequirementsV1,
    ) -> Result<Self, LocalShapeSupportPlanError> {
        let mut roots = Vec::with_capacity(requirements.roots().len());
        for requirement in requirements.roots() {
            let ty = module
                .exact_type_identities
                .type_for_identity(requirement.exact())
                .ok_or(LocalShapeSupportPlanError::MissingExactType(
                    requirement.exact(),
                ))?;
            let record = &module.exact_type_identities[ty];
            if record.key() != &ExactTypeKey::Nominal(requirement.source()) {
                return Err(LocalShapeSupportPlanError::IdentityMismatch {
                    source: requirement.source(),
                    exact: requirement.exact(),
                });
            }
            let source = requirement.source();
            let declaration = if source == CoreBuiltinNominal::Unit.identity_record().id() {
                CoreBuiltinNominal::Unit.identity_record().key().clone()
            } else if source == CoreBuiltinNominal::Any.identity_record().id() {
                CoreBuiltinNominal::Any.identity_record().key().clone()
            } else {
                let mut declarations = module
                    .structs
                    .iter()
                    .map(|(_, declaration)| &declaration.origin)
                    .chain(
                        module
                            .enums
                            .iter()
                            .map(|(_, declaration)| &declaration.origin),
                    )
                    .chain(
                        module
                            .classes
                            .iter()
                            .map(|(_, declaration)| &declaration.origin),
                    )
                    .chain(
                        module
                            .interfaces
                            .iter()
                            .map(|(_, declaration)| &declaration.origin),
                    )
                    .chain(
                        module
                            .objects
                            .iter()
                            .map(|(_, declaration)| &declaration.origin),
                    )
                    .filter_map(|origin| {
                        (origin.concrete_type_id() == Some(source))
                            .then(|| origin.source())
                            .flatten()
                            .map(|identity| identity.declaration())
                    });
                let declaration = declarations
                    .next()
                    .ok_or(LocalShapeSupportPlanError::MissingSourceNominal(source))?;
                if declarations.next().is_some() {
                    return Err(LocalShapeSupportPlanError::AmbiguousSourceNominal(source));
                }
                declaration.clone()
            };
            if declaration.origin() != module.cone {
                return Err(LocalShapeSupportPlanError::InvalidSourceNominal(source));
            }
            let boxed_value = match declaration.declaration_kind() {
                SourceDeclarationKind::Struct | SourceDeclarationKind::Enum => {
                    LocalBoxedValueRequirement::Required
                }
                SourceDeclarationKind::Class
                | SourceDeclarationKind::Interface
                | SourceDeclarationKind::Object
                | SourceDeclarationKind::AnnotationClass => {
                    LocalBoxedValueRequirement::NotApplicable
                }
                _ => {
                    return Err(LocalShapeSupportPlanError::InvalidSourceNominal(source));
                }
            };
            roots.push(LocalShapeSupportRoot {
                declaration,
                source,
                exact: requirement.exact(),
                ty,
                boxed_value,
            });
        }
        Ok(Self { roots })
    }

    pub fn roots(&self) -> &[LocalShapeSupportRoot] {
        &self.roots
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalShapeSupportPlanError {
    MissingExactType(PersistentExactTypeId),
    MissingSourceNominal(PersistentTypeId),
    AmbiguousSourceNominal(PersistentTypeId),
    InvalidSourceNominal(PersistentTypeId),
    IdentityMismatch {
        source: PersistentTypeId,
        exact: PersistentExactTypeId,
    },
}

impl fmt::Display for LocalShapeSupportPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot project shape support into LocalConcrete HIR: {self:?}"
        )
    }
}

impl std::error::Error for LocalShapeSupportPlanError {}

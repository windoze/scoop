use scoop_identity::{
    ConeIdentity, DeclarationScope, DefinitionOrigin, PersistentTypeAliasId, SourceDeclarationKey,
    SourceDeclarationKind,
};

use super::TypeAliasInterfaceRecordV1;
use crate::{
    NominalInterfaceShapeAuthority, PublicLookupAccessV1, SignatureBinderScopeV1, TypeAliasTargetV1,
};

mod errors;

pub use errors::TypeAliasInterfaceSemanticValidationError;

/// Canonical definition-side facts for one public type-alias declaration.
///
/// A successful authority lookup also proves that the declaration is
/// explicitly public and has a universal effective lookup domain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAliasDeclarationSourceV1 {
    declaration: SourceDeclarationKey,
    access: PublicLookupAccessV1,
    definition_origin: DefinitionOrigin,
}

impl TypeAliasDeclarationSourceV1 {
    pub fn new(
        declaration: SourceDeclarationKey,
        access: PublicLookupAccessV1,
        definition_origin: DefinitionOrigin,
    ) -> Self {
        Self {
            declaration,
            access,
            definition_origin,
        }
    }

    pub const fn declaration(&self) -> &SourceDeclarationKey {
        &self.declaration
    }

    pub const fn access(&self) -> PublicLookupAccessV1 {
        self.access
    }

    pub const fn definition_origin(&self) -> &DefinitionOrigin {
        &self.definition_origin
    }
}

/// Supplies current-artifact source facts and nominal shapes for aliases.
pub trait TypeAliasInterfaceSemanticAuthority<E>: NominalInterfaceShapeAuthority<E> {
    fn current_cone(&self) -> ConeIdentity;

    fn type_alias_declaration_source(
        &mut self,
        alias: PersistentTypeAliasId,
    ) -> Result<TypeAliasDeclarationSourceV1, E>;
}

impl TypeAliasInterfaceRecordV1 {
    pub fn validate_semantics<A, E>(
        &self,
        authority: &mut A,
    ) -> Result<(), TypeAliasInterfaceSemanticValidationError<E>>
    where
        A: TypeAliasInterfaceSemanticAuthority<E>,
    {
        let current_cone = authority.current_cone();
        let source = authority
            .type_alias_declaration_source(self.alias)
            .map_err(TypeAliasInterfaceSemanticValidationError::Declaration)?;
        self.validate_declaration_source(current_cone, &source)?;

        if let TypeAliasTargetV1::Signature(target) = &self.target {
            SignatureBinderScopeV1::for_declaration(0, None)
                .validate_signature_semantics(target, authority)
                .map_err(TypeAliasInterfaceSemanticValidationError::Target)?;
        }
        Ok(())
    }

    fn validate_declaration_source<E>(
        &self,
        current_cone: ConeIdentity,
        source: &TypeAliasDeclarationSourceV1,
    ) -> Result<(), TypeAliasInterfaceSemanticValidationError<E>> {
        let declaration = source.declaration();
        if declaration.declaration_kind() != SourceDeclarationKind::TypeAlias {
            return Err(TypeAliasInterfaceSemanticValidationError::DeclarationKind {
                actual: declaration.declaration_kind(),
            });
        }
        let actual_alias = PersistentTypeAliasId::from_source_declaration(declaration)
            .map_err(TypeAliasInterfaceSemanticValidationError::DeclarationIdentity)?;
        if actual_alias != self.alias {
            return Err(
                TypeAliasInterfaceSemanticValidationError::DeclarationIdentityMismatch {
                    expected: self.alias,
                    actual: actual_alias,
                },
            );
        }
        if declaration.origin() != current_cone {
            return Err(TypeAliasInterfaceSemanticValidationError::DeclarationCone {
                expected: current_cone,
                actual: declaration.origin(),
            });
        }
        if !declaration.owners().owners().is_empty() {
            return Err(
                TypeAliasInterfaceSemanticValidationError::NestedDeclaration {
                    owner_depth: declaration.owners().owners().len(),
                },
            );
        }
        if declaration.scope() != &DeclarationScope::ConeWide {
            return Err(
                TypeAliasInterfaceSemanticValidationError::DeclarationScope {
                    actual: declaration.scope().clone(),
                },
            );
        }
        if self.access != source.access() {
            return Err(TypeAliasInterfaceSemanticValidationError::Access {
                expected: source.access(),
                actual: self.access,
            });
        }

        let expected_origin = source.definition_origin();
        let actual_cone = expected_origin.source().cone();
        if actual_cone != current_cone {
            return Err(
                TypeAliasInterfaceSemanticValidationError::DefinitionOriginCone {
                    expected: current_cone,
                    actual: actual_cone,
                },
            );
        }
        let actual_origin = self.definition_origin.origin();
        if actual_origin != expected_origin {
            return Err(
                TypeAliasInterfaceSemanticValidationError::DefinitionOriginMismatch {
                    expected: Box::new(expected_origin.clone()),
                    actual: Box::new(actual_origin.clone()),
                },
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

use scoop_identity::{
    CanonicalIdentifier, DeclarationName, DeclarationScope, DefinitionOwnerAtom,
    NominalDeclarationOwner,
};

use super::{
    NominalArenaKind, NominalInterfaceBuildError, NominalProjection, ProjectedNominalHeader,
};
use crate::{
    ExportHir, HirNominalIdentity, HirSourceNominalIdentity, NominalOwner, PublicNominalKindV1,
    TypeParamDecl,
};

#[derive(Clone, Copy)]
pub(super) enum LocalNominalId {
    Class(crate::ClassId),
    Interface(crate::InterfaceId),
    Struct(crate::StructId),
    Enum(crate::EnumId),
    Object(crate::ObjectId),
}

impl NominalProjection<'_> {
    pub(super) fn project_header(
        &self,
        local: LocalNominalId,
        name: &str,
        owner: Option<NominalOwner>,
        access: &crate::NominalAccess,
        parameters: &[TypeParamDecl],
    ) -> Result<ProjectedNominalHeader, NominalInterfaceBuildError> {
        let identity = local.identity(self.export).ok_or_else(|| {
            let (kind, index) = local.location();
            NominalInterfaceBuildError::MissingNominalIdentity { kind, index }
        })?;
        let source = identity.source().ok_or_else(|| {
            let (kind, index) = local.location();
            NominalInterfaceBuildError::GeneratedNominalIdentity { kind, index }
        })?;
        let declaration = source_nominal_id(source);
        let key = source.declaration();
        if key.origin() != self.export.cone {
            return Err(NominalInterfaceBuildError::ForeignDeclaration {
                declaration,
                expected: self.export.cone,
                actual: key.origin(),
            });
        }
        if key.scope() != &DeclarationScope::ConeWide {
            return Err(NominalInterfaceBuildError::InvalidDeclarationScope(
                declaration,
            ));
        }
        if key.declaration_kind() != local.kind().source_kind() {
            return Err(NominalInterfaceBuildError::DeclarationKind {
                declaration,
                expected: local.kind(),
                actual: key.declaration_kind(),
            });
        }
        let canonical_name = CanonicalIdentifier::new(name).map_err(|source| {
            let (kind, index) = local.location();
            NominalInterfaceBuildError::InvalidDeclarationName {
                kind,
                index,
                source,
            }
        })?;
        if key.name() != &DeclarationName::Named(canonical_name) {
            return Err(NominalInterfaceBuildError::DeclarationNameMismatch(
                declaration,
            ));
        }
        let actual = u32::try_from(parameters.len()).map_err(|_| {
            NominalInterfaceBuildError::TypeParameterArity {
                declaration,
                expected: key.duplicate_signature().type_parameter_count(),
                actual: u32::MAX,
            }
        })?;
        let expected = key.duplicate_signature().type_parameter_count();
        if actual != expected {
            return Err(NominalInterfaceBuildError::TypeParameterArity {
                declaration,
                expected,
                actual,
            });
        }
        if access.declared != crate::DeclaredVisibility::Public || !access.lookup.0.is_universal() {
            return Err(NominalInterfaceBuildError::InvalidPublicAccess(declaration));
        }
        let expected_owner = owner
            .map(|owner| self.persistent_owner_atom(declaration, owner))
            .transpose()?;
        let actual_owner = key.owners().owners().last().cloned();
        if actual_owner != expected_owner {
            return Err(NominalInterfaceBuildError::LexicalOwnerMismatch {
                declaration,
                expected: expected_owner,
                actual: actual_owner,
            });
        }
        let binders = self
            .signatures
            .binder_frame(parameters, 0)
            .map_err(|source| NominalInterfaceBuildError::Signature {
                declaration,
                source,
            })?;
        let type_parameters = self
            .signatures
            .project_binder_list(parameters, &binders)
            .map_err(|source| NominalInterfaceBuildError::Signature {
                declaration,
                source,
            })?;
        Ok(ProjectedNominalHeader {
            declaration,
            type_parameters,
            binders,
        })
    }

    fn persistent_owner_atom(
        &self,
        declaration: NominalDeclarationOwner,
        owner: NominalOwner,
    ) -> Result<DefinitionOwnerAtom, NominalInterfaceBuildError> {
        let (local, identity) = match owner {
            NominalOwner::Class(id) => (
                LocalNominalId::Class(id),
                self.export.nominal_identities.get_class(id),
            ),
            NominalOwner::Interface(id) => (
                LocalNominalId::Interface(id),
                self.export.nominal_identities.get_interface(id),
            ),
            NominalOwner::Struct(id) => (
                LocalNominalId::Struct(id),
                self.export.nominal_identities.get_struct(id),
            ),
            NominalOwner::Enum(id) => (
                LocalNominalId::Enum(id),
                self.export.nominal_identities.get_enum(id),
            ),
            NominalOwner::Object(id) => (
                LocalNominalId::Object(id),
                self.export.nominal_identities.get_object(id),
            ),
        };
        let identity = identity.ok_or_else(|| {
            let (owner, index) = local.location();
            NominalInterfaceBuildError::UnknownLexicalOwner {
                declaration,
                owner,
                index,
            }
        })?;
        identity
            .source()
            .map(HirSourceNominalIdentity::definition_owner)
            .ok_or_else(|| {
                let (owner, index) = local.location();
                NominalInterfaceBuildError::GeneratedLexicalOwner {
                    declaration,
                    owner,
                    index,
                }
            })
    }
}

impl LocalNominalId {
    pub(super) fn kind(self) -> PublicNominalKindV1 {
        match self {
            Self::Class(_) => PublicNominalKindV1::Class,
            Self::Interface(_) => PublicNominalKindV1::Interface,
            Self::Struct(_) => PublicNominalKindV1::Struct,
            Self::Enum(_) => PublicNominalKindV1::Enum,
            Self::Object(_) => PublicNominalKindV1::Object,
        }
    }

    pub(super) fn location(self) -> (NominalArenaKind, u32) {
        match self {
            Self::Class(id) => (NominalArenaKind::Class, super::raw_index(id)),
            Self::Interface(id) => (NominalArenaKind::Interface, super::raw_index(id)),
            Self::Struct(id) => (NominalArenaKind::Struct, super::raw_index(id)),
            Self::Enum(id) => (NominalArenaKind::Enum, super::raw_index(id)),
            Self::Object(id) => (NominalArenaKind::Object, super::raw_index(id)),
        }
    }

    pub(super) fn identity(self, export: &ExportHir) -> Option<&HirNominalIdentity> {
        match self {
            Self::Class(id) => export.nominal_identities.get_class(id),
            Self::Interface(id) => export.nominal_identities.get_interface(id),
            Self::Struct(id) => export.nominal_identities.get_struct(id),
            Self::Enum(id) => export.nominal_identities.get_enum(id),
            Self::Object(id) => export.nominal_identities.get_object(id),
        }
    }

    pub(super) fn owner(self) -> NominalOwner {
        match self {
            Self::Class(id) => NominalOwner::Class(id),
            Self::Interface(id) => NominalOwner::Interface(id),
            Self::Struct(id) => NominalOwner::Struct(id),
            Self::Enum(id) => NominalOwner::Enum(id),
            Self::Object(id) => NominalOwner::Object(id),
        }
    }
}

pub(super) fn source_nominal_id(source: &HirSourceNominalIdentity) -> NominalDeclarationOwner {
    match source {
        HirSourceNominalIdentity::Concrete(record) => {
            NominalDeclarationOwner::Concrete(record.id())
        }
        HirSourceNominalIdentity::Generic(record) => {
            NominalDeclarationOwner::GenericTemplate(record.id())
        }
    }
}

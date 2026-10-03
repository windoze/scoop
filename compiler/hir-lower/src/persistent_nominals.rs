//! Establish declaration identities before constructing their applications.

use std::fmt;

mod storage;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, PackagePath, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

use crate::{Lowerer, Owner, namespace::TopLevelLookupLayer};

pub(crate) struct NominalIdentityInput<'a> {
    pub(crate) name: &'a str,
    pub(crate) parent: Option<Owner>,
    pub(crate) access: hir::DeclaredVisibility,
    pub(crate) type_parameter_count: usize,
    pub(crate) kind: SourceNominalKind,
    pub(crate) file: usize,
    pub(crate) span: Span,
}

impl Lowerer {
    pub(crate) fn prepare_nominal_identity(
        &mut self,
        input: NominalIdentityInput<'_>,
    ) -> Option<hir::HirNominalIdentity> {
        match self.source_nominal_identity(&input) {
            Ok(identity) => Some(identity),
            Err(detail) => {
                let error = PersistentNominalIdentityError {
                    file: input.file,
                    span: input.span,
                    detail,
                };
                let mut diagnostic = scoop_ast::Diagnostic::at(error.span(), error.to_string());
                diagnostic.file = error.file();
                self.diagnostics.push(diagnostic);
                None
            }
        }
    }

    fn source_nominal_identity(
        &self,
        input: &NominalIdentityInput<'_>,
    ) -> Result<hir::HirNominalIdentity, PersistentNominalIdentityErrorDetail> {
        use PersistentNominalIdentityErrorDetail as Error;

        let source = self.visibility_file(input.file);
        let package = match self.top_level_namespaces.source_namespace(input.file) {
            TopLevelLookupLayer::CorePrelude => PackagePath::root(),
            TopLevelLookupLayer::CurrentPackage(package) => {
                let segments = self
                    .top_level_namespaces
                    .package_segments(package)
                    .into_iter()
                    .map(CanonicalIdentifier::new)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(Error::InvalidName)?;
                PackagePath::from_segments(segments)
            }
        };
        let owners = if let Some(parent) = input.parent {
            let parent = self
                .nominal_identity(parent)
                .source()
                .ok_or(Error::GeneratedDeclarationOwner)?;
            let mut owners = parent.declaration().owners().owners().to_vec();
            owners.push(parent.definition_owner());
            DefinitionOwnerChain::from_outer_to_inner(owners)
        } else {
            DefinitionOwnerChain::top_level()
        };
        let scope = if input.parent.is_none() && input.access == hir::DeclaredVisibility::Private {
            DeclarationScope::SourceScoped(source.clone())
        } else {
            DeclarationScope::ConeWide
        };
        let site = SourceDeclarationSite::new(source.cone(), package, owners, scope)
            .map_err(Error::InvalidSite)?;
        let name = CanonicalIdentifier::new(input.name).map_err(Error::InvalidName)?;
        let arity =
            u32::try_from(input.type_parameter_count).map_err(|_| Error::TooManyTypeParameters)?;
        hir::HirNominalIdentity::from_source_declaration(SourceDeclarationKey::nominal(
            site, name, input.kind, arity,
        ))
        .map_err(Error::InvalidIdentity)
    }

    pub(crate) fn register_nominal_identity(
        &mut self,
        owner: Owner,
        identity: hir::HirNominalIdentity,
    ) {
        let previous = self.nominal_owners.insert(identity.declaration_id(), owner);
        assert!(
            previous.is_none(),
            "a declaration has one original nominal identity"
        );
        let previous = self.nominal_declaration_identities.insert(owner, identity);
        assert!(
            previous.is_none(),
            "nominal identities are established once"
        );
    }

    pub(crate) fn nominal_identity(&self, owner: Owner) -> &hir::HirNominalIdentity {
        &self.nominal_declaration_identities[&owner]
    }

    /// Publish an arena-aligned view of records already used by applications.
    pub(crate) fn establish_nominal_identities(
        &mut self,
    ) -> Result<(), PersistentNominalIdentityError> {
        let identities = hir::HirNominalIdentities::checked(
            &self.structs,
            self.structs
                .iter()
                .map(|(id, _)| self.nominal_identity(Owner::Struct(id)).clone())
                .collect(),
            &self.enums,
            self.enums
                .iter()
                .map(|(id, _)| self.nominal_identity(Owner::Enum(id)).clone())
                .collect(),
            &self.classes,
            self.classes
                .iter()
                .map(|(id, _)| self.nominal_identity(Owner::Class(id)).clone())
                .collect(),
            &self.interfaces,
            self.interfaces
                .iter()
                .map(|(id, _)| self.nominal_identity(Owner::Interface(id)).clone())
                .collect(),
            &self.objects,
            self.objects
                .iter()
                .map(|(id, _)| self.nominal_identity(Owner::Object(id)).clone())
                .collect(),
        )
        .map_err(|error| PersistentNominalIdentityError {
            file: 0,
            span: Span::new(0, 0),
            detail: PersistentNominalIdentityErrorDetail::MisalignedTable(error),
        })?;
        self.nominal_identities = Some(identities);
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct PersistentNominalIdentityError {
    file: usize,
    span: Span,
    detail: PersistentNominalIdentityErrorDetail,
}

impl PersistentNominalIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentNominalIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent nominal identity: {}",
            self.detail
        )
    }
}

impl std::error::Error for PersistentNominalIdentityError {}

#[derive(Debug)]
enum PersistentNominalIdentityErrorDetail {
    InvalidName(scoop_identity::CanonicalIdentifierError),
    InvalidSite(scoop_identity::SourceDeclarationKeyError),
    InvalidIdentity(hir::HirNominalIdentityError),
    TooManyTypeParameters,
    GeneratedDeclarationOwner,
    MisalignedTable(hir::HirNominalIdentityTableError),
}

impl fmt::Display for PersistentNominalIdentityErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(error) => error.fmt(formatter),
            Self::InvalidSite(error) => error.fmt(formatter),
            Self::InvalidIdentity(error) => error.fmt(formatter),
            Self::TooManyTypeParameters => {
                formatter.write_str("type parameter count exceeds the identity schema")
            }
            Self::GeneratedDeclarationOwner => {
                formatter.write_str("a source declaration cannot be owned by a generated nominal")
            }
            Self::MisalignedTable(error) => error.fmt(formatter),
        }
    }
}

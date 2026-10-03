use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, PackagePath, SourceDeclarationKey,
    SourceDeclarationSite,
};

use crate::{Lowerer, namespace::TopLevelLookupLayer};

pub(crate) fn build(
    lowerer: &Lowerer,
) -> Result<hir::HirTypeAliasIdentities, PersistentTypeAliasIdentityError> {
    let declarations = lowerer
        .type_aliases
        .iter()
        .map(|(_, alias)| declaration(lowerer, alias))
        .collect::<Result<Vec<_>, _>>()?;
    hir::HirTypeAliasIdentities::from_declarations(&lowerer.type_aliases, declarations).map_err(
        |error| PersistentTypeAliasIdentityError {
            file: 0,
            span: Span { start: 0, end: 0 },
            detail: PersistentTypeAliasIdentityErrorDetail::InvalidIdentity(error),
        },
    )
}

fn declaration(
    lowerer: &Lowerer,
    alias: &hir::TypeAliasDecl,
) -> Result<SourceDeclarationKey, PersistentTypeAliasIdentityError> {
    let file = usize::try_from(alias.origin.file).map_err(|_| {
        failure(
            0,
            alias,
            PersistentTypeAliasIdentityErrorDetail::UnknownSourceFile(alias.origin.file),
        )
    })?;
    if file >= lowerer.intrinsic_sources.len() {
        return Err(failure(
            0,
            alias,
            PersistentTypeAliasIdentityErrorDetail::UnknownSourceFile(alias.origin.file),
        ));
    }
    let source = lowerer.visibility_file(file);
    let package = match lowerer.top_level_namespaces.source_namespace(file) {
        TopLevelLookupLayer::CorePrelude => PackagePath::root(),
        TopLevelLookupLayer::CurrentPackage(package) => {
            let segments = lowerer
                .top_level_namespaces
                .package_segments(package)
                .into_iter()
                .map(CanonicalIdentifier::new)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    failure(
                        file,
                        alias,
                        PersistentTypeAliasIdentityErrorDetail::InvalidName(error),
                    )
                })?;
            PackagePath::from_segments(segments)
        }
    };
    let scope = if alias.access.declared == hir::DeclaredVisibility::Private {
        DeclarationScope::SourceScoped(source.clone())
    } else {
        DeclarationScope::ConeWide
    };
    let site = SourceDeclarationSite::new(
        source.cone(),
        package,
        DefinitionOwnerChain::top_level(),
        scope,
    )
    .map_err(|error| {
        failure(
            file,
            alias,
            PersistentTypeAliasIdentityErrorDetail::InvalidSite(error),
        )
    })?;
    let name = CanonicalIdentifier::new(&alias.name).map_err(|error| {
        failure(
            file,
            alias,
            PersistentTypeAliasIdentityErrorDetail::InvalidName(error),
        )
    })?;
    Ok(SourceDeclarationKey::type_alias(site, name))
}

#[derive(Debug)]
pub(crate) struct PersistentTypeAliasIdentityError {
    file: usize,
    span: Span,
    detail: PersistentTypeAliasIdentityErrorDetail,
}

impl PersistentTypeAliasIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentTypeAliasIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent type-alias identity: {}",
            self.detail
        )
    }
}

impl std::error::Error for PersistentTypeAliasIdentityError {}

#[derive(Debug)]
enum PersistentTypeAliasIdentityErrorDetail {
    UnknownSourceFile(u32),
    InvalidName(scoop_identity::CanonicalIdentifierError),
    InvalidSite(scoop_identity::SourceDeclarationKeyError),
    InvalidIdentity(hir::HirTypeAliasIdentityError),
}

impl fmt::Display for PersistentTypeAliasIdentityErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSourceFile(file) => {
                write!(
                    formatter,
                    "declaration refers to unknown source file {file}"
                )
            }
            Self::InvalidName(error) => error.fmt(formatter),
            Self::InvalidSite(error) => error.fmt(formatter),
            Self::InvalidIdentity(error) => error.fmt(formatter),
        }
    }
}

fn failure(
    file: usize,
    alias: &hir::TypeAliasDecl,
    detail: PersistentTypeAliasIdentityErrorDetail,
) -> PersistentTypeAliasIdentityError {
    PersistentTypeAliasIdentityError {
        file,
        span: alias.origin.span,
        detail,
    }
}

#[cfg(test)]
mod tests;

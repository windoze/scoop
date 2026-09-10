use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, PackagePath, SourceDeclarationKey,
    SourceDeclarationSite,
};

use crate::persistent_types::{SignatureBinder, SignatureTypeMapper, SignatureTypeMappingError};
use crate::{Lowerer, Owner, namespace::TopLevelLookupLayer};

pub(crate) fn build(
    lowerer: &Lowerer,
    nominals: &hir::HirNominalIdentities,
    intrinsic_core: &hir::IntrinsicTypeCore,
) -> Result<hir::HirPropertyIdentities, PersistentPropertyIdentityError> {
    PropertyIdentityBuilder {
        lowerer,
        nominals,
        type_mapper: SignatureTypeMapper::new(lowerer, nominals, intrinsic_core),
    }
    .build()
}

#[derive(Debug)]
pub(crate) struct PersistentPropertyIdentityError {
    file: usize,
    span: Span,
    detail: PersistentPropertyIdentityErrorDetail,
}

impl PersistentPropertyIdentityError {
    pub(crate) const fn file(&self) -> usize {
        self.file
    }

    pub(crate) const fn span(&self) -> Span {
        self.span
    }
}

impl fmt::Display for PersistentPropertyIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot derive persistent property identity: {}",
            self.detail
        )
    }
}

impl std::error::Error for PersistentPropertyIdentityError {}

#[derive(Debug)]
enum PersistentPropertyIdentityErrorDetail {
    MissingSourceFile,
    InvalidName(scoop_identity::CanonicalIdentifierError),
    InvalidSite(scoop_identity::SourceDeclarationKeyError),
    InvalidIdentity(scoop_identity::SourceDeclarationIdentityError),
    InvalidSignatureType(SignatureTypeMappingError),
    TooManyTypeParameters,
    UnknownExtension,
    GeneratedDeclarationOwner,
    OwnerSiteMismatch,
    MisalignedTable(hir::HirPropertyIdentityTableError),
}

impl fmt::Display for PersistentPropertyIdentityErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSourceFile => formatter.write_str("declaration has no source file"),
            Self::InvalidName(error) => error.fmt(formatter),
            Self::InvalidSite(error) => error.fmt(formatter),
            Self::InvalidIdentity(error) => error.fmt(formatter),
            Self::InvalidSignatureType(error) => error.fmt(formatter),
            Self::TooManyTypeParameters => {
                formatter.write_str("type parameter count exceeds the identity schema")
            }
            Self::UnknownExtension => {
                formatter.write_str("property refers to an unknown extension declaration")
            }
            Self::GeneratedDeclarationOwner => {
                formatter.write_str("a source property cannot be owned by a generated nominal")
            }
            Self::OwnerSiteMismatch => formatter.write_str(
                "property source Cone or package differs from its nominal declaration owner",
            ),
            Self::MisalignedTable(error) => error.fmt(formatter),
        }
    }
}

struct PropertyIdentityBuilder<'a> {
    lowerer: &'a Lowerer,
    nominals: &'a hir::HirNominalIdentities,
    type_mapper: SignatureTypeMapper<'a>,
}

impl PropertyIdentityBuilder<'_> {
    fn build(self) -> Result<hir::HirPropertyIdentities, PersistentPropertyIdentityError> {
        let identities = self
            .lowerer
            .properties
            .iter()
            .map(|(id, property)| self.identity(id, property))
            .collect::<Result<Vec<_>, _>>()?;
        hir::HirPropertyIdentities::checked(
            &self.lowerer.properties,
            identities,
            &self.lowerer.extension_properties,
        )
        .map_err(|error| self.table_failure(error))
    }

    fn identity(
        &self,
        id: hir::PropertyId,
        property: &hir::Property,
    ) -> Result<hir::HirPropertyIdentity, PersistentPropertyIdentityError> {
        let file = self.source_file(id)?;
        let site = self.declaration_site(id, property, file)?;
        let name = CanonicalIdentifier::new(&property.name).map_err(|error| {
            self.failure(
                id,
                PersistentPropertyIdentityErrorDetail::InvalidName(error),
            )
        })?;
        match property.owner {
            hir::PropertyOwner::Extension(extension_id) => {
                if raw_index(extension_id) as usize >= self.lowerer.extension_properties.len() {
                    return Err(
                        self.failure(id, PersistentPropertyIdentityErrorDetail::UnknownExtension)
                    );
                }
                let extension = &self.lowerer.extension_properties[extension_id];
                let mut binders = Vec::with_capacity(extension.type_params.len());
                for (index, parameter) in extension.type_params.iter().enumerate() {
                    binders.push(SignatureBinder {
                        parameter: parameter.id,
                        depth: 0,
                        index: u32::try_from(index).map_err(|_| {
                            self.failure(
                                id,
                                PersistentPropertyIdentityErrorDetail::TooManyTypeParameters,
                            )
                        })?,
                    });
                }
                let receiver = self
                    .type_mapper
                    .map(extension.receiver_ty, &binders)
                    .map_err(|error| {
                        self.failure(
                            id,
                            PersistentPropertyIdentityErrorDetail::InvalidSignatureType(error),
                        )
                    })?;
                let count = u32::try_from(extension.type_params.len()).map_err(|_| {
                    self.failure(
                        id,
                        PersistentPropertyIdentityErrorDetail::TooManyTypeParameters,
                    )
                })?;
                let declaration =
                    SourceDeclarationKey::extension_property(site, name, count, receiver);
                hir::HirPropertyIdentity::from_extension_declaration(declaration).map_err(|error| {
                    self.failure(
                        id,
                        PersistentPropertyIdentityErrorDetail::InvalidIdentity(error),
                    )
                })
            }
            hir::PropertyOwner::TopLevel
            | hir::PropertyOwner::Class(_)
            | hir::PropertyOwner::Struct(_)
            | hir::PropertyOwner::Enum(_)
            | hir::PropertyOwner::Interface(_)
            | hir::PropertyOwner::Object(_) => {
                let declaration = SourceDeclarationKey::property(site, name);
                hir::HirPropertyIdentity::from_ordinary_declaration(declaration).map_err(|error| {
                    self.failure(
                        id,
                        PersistentPropertyIdentityErrorDetail::InvalidIdentity(error),
                    )
                })
            }
        }
    }

    fn declaration_site(
        &self,
        id: hir::PropertyId,
        property: &hir::Property,
        file: usize,
    ) -> Result<SourceDeclarationSite, PersistentPropertyIdentityError> {
        let source = &self.lowerer.visibility_file(file).source;
        let package = self.package(id, file)?;
        let owners = match property.owner {
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_) => {
                DefinitionOwnerChain::top_level()
            }
            hir::PropertyOwner::Class(owner) => {
                self.member_owners(id, Owner::Class(owner), source, &package)?
            }
            hir::PropertyOwner::Struct(owner) => {
                self.member_owners(id, Owner::Struct(owner), source, &package)?
            }
            hir::PropertyOwner::Enum(owner) => {
                self.member_owners(id, Owner::Enum(owner), source, &package)?
            }
            hir::PropertyOwner::Interface(owner) => {
                self.member_owners(id, Owner::Interface(owner), source, &package)?
            }
            hir::PropertyOwner::Object(owner) => {
                self.member_owners(id, Owner::Object(owner), source, &package)?
            }
        };
        let scope = if matches!(
            property.owner,
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_)
        ) && property.access.declared == hir::DeclaredVisibility::Private
        {
            DeclarationScope::SourceScoped(source.clone())
        } else {
            DeclarationScope::ConeWide
        };
        SourceDeclarationSite::new(source.cone(), package, owners, scope).map_err(|error| {
            self.failure(
                id,
                PersistentPropertyIdentityErrorDetail::InvalidSite(error),
            )
        })
    }

    fn member_owners(
        &self,
        property: hir::PropertyId,
        owner: Owner,
        source: &scoop_identity::SourceIdentity,
        package: &PackagePath,
    ) -> Result<DefinitionOwnerChain, PersistentPropertyIdentityError> {
        let identity = match owner {
            Owner::Class(id) => &self.nominals[id],
            Owner::Interface(id) => &self.nominals[id],
            Owner::Struct(id) => &self.nominals[id],
            Owner::Enum(id) => &self.nominals[id],
            Owner::Object(id) => &self.nominals[id],
        };
        let source_owner = identity.source().ok_or_else(|| {
            self.failure(
                property,
                PersistentPropertyIdentityErrorDetail::GeneratedDeclarationOwner,
            )
        })?;
        let declaration = source_owner.declaration();
        if declaration.origin() != source.cone() || declaration.package() != package {
            return Err(self.failure(
                property,
                PersistentPropertyIdentityErrorDetail::OwnerSiteMismatch,
            ));
        }
        let mut owners = declaration.owners().owners().to_vec();
        owners.push(source_owner.definition_owner());
        Ok(DefinitionOwnerChain::from_outer_to_inner(owners))
    }

    fn package(
        &self,
        property: hir::PropertyId,
        file: usize,
    ) -> Result<PackagePath, PersistentPropertyIdentityError> {
        match self.lowerer.top_level_namespaces.source_namespace(file) {
            TopLevelLookupLayer::CorePrelude => Ok(PackagePath::root()),
            TopLevelLookupLayer::CurrentPackage(package) => self
                .lowerer
                .top_level_namespaces
                .package_segments(package)
                .into_iter()
                .map(CanonicalIdentifier::new)
                .collect::<Result<Vec<_>, _>>()
                .map(PackagePath::from_segments)
                .map_err(|error| {
                    self.failure(
                        property,
                        PersistentPropertyIdentityErrorDetail::InvalidName(error),
                    )
                }),
        }
    }

    fn source_file(
        &self,
        property: hir::PropertyId,
    ) -> Result<usize, PersistentPropertyIdentityError> {
        let file = match self.lowerer.properties[property].owner {
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_) => {
                self.lowerer.property_files.get(&property)
            }
            hir::PropertyOwner::Class(owner) => self.lowerer.class_files.get(&owner),
            hir::PropertyOwner::Struct(owner) => self.lowerer.struct_files.get(&owner),
            hir::PropertyOwner::Enum(owner) => self.lowerer.enum_files.get(&owner),
            hir::PropertyOwner::Interface(owner) => self.lowerer.interface_files.get(&owner),
            hir::PropertyOwner::Object(owner) => self.lowerer.object_files.get(&owner),
        };
        file.copied().ok_or_else(|| {
            self.failure(
                property,
                PersistentPropertyIdentityErrorDetail::MissingSourceFile,
            )
        })
    }

    fn failure(
        &self,
        property: hir::PropertyId,
        detail: PersistentPropertyIdentityErrorDetail,
    ) -> PersistentPropertyIdentityError {
        let declaration = &self.lowerer.properties[property];
        PersistentPropertyIdentityError {
            file: match declaration.owner {
                hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_) => {
                    self.lowerer.property_files.get(&property)
                }
                hir::PropertyOwner::Class(owner) => self.lowerer.class_files.get(&owner),
                hir::PropertyOwner::Struct(owner) => self.lowerer.struct_files.get(&owner),
                hir::PropertyOwner::Enum(owner) => self.lowerer.enum_files.get(&owner),
                hir::PropertyOwner::Interface(owner) => self.lowerer.interface_files.get(&owner),
                hir::PropertyOwner::Object(owner) => self.lowerer.object_files.get(&owner),
            }
            .copied()
            .unwrap_or(0),
            span: declaration.span,
            detail,
        }
    }

    fn table_failure(
        &self,
        error: hir::HirPropertyIdentityTableError,
    ) -> PersistentPropertyIdentityError {
        PersistentPropertyIdentityError {
            file: 0,
            span: Span { start: 0, end: 0 },
            detail: PersistentPropertyIdentityErrorDetail::MisalignedTable(error),
        }
    }
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}

#[cfg(test)]
mod tests;

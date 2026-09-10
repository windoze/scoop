use std::collections::{HashMap, HashSet};
use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::{
    CanonicalIdentifier, DeclarationScope, DefinitionOwnerChain, GeneratedNominalKey, PackagePath,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

use crate::{Lowerer, Owner, namespace::TopLevelLookupLayer};

pub(crate) fn build(
    lowerer: &Lowerer,
) -> Result<hir::HirNominalIdentities, PersistentNominalIdentityError> {
    NominalIdentityBuilder::new(lowerer).build()
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
    MissingSourceFile,
    InvalidName(scoop_identity::CanonicalIdentifierError),
    InvalidSite(scoop_identity::SourceDeclarationKeyError),
    InvalidIdentity(hir::HirNominalIdentityError),
    TooManyTypeParameters,
    CyclicOwner,
    GeneratedDeclarationOwner,
    GenericObject,
    MisalignedTable(hir::HirNominalIdentityTableError),
}

impl fmt::Display for PersistentNominalIdentityErrorDetail {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSourceFile => formatter.write_str("declaration has no source file"),
            Self::InvalidName(error) => error.fmt(formatter),
            Self::InvalidSite(error) => error.fmt(formatter),
            Self::InvalidIdentity(error) => error.fmt(formatter),
            Self::TooManyTypeParameters => {
                formatter.write_str("type parameter count exceeds the identity schema")
            }
            Self::CyclicOwner => formatter.write_str("nominal owner relation is cyclic"),
            Self::GeneratedDeclarationOwner => {
                formatter.write_str("a source declaration cannot be owned by a generated nominal")
            }
            Self::GenericObject => {
                formatter.write_str("an object declaration cannot have a generic type identity")
            }
            Self::MisalignedTable(error) => error.fmt(formatter),
        }
    }
}

struct NominalIdentityBuilder<'a> {
    lowerer: &'a Lowerer,
    resolved: HashMap<Owner, hir::HirNominalIdentity>,
    visiting: HashSet<Owner>,
}

impl<'a> NominalIdentityBuilder<'a> {
    fn new(lowerer: &'a Lowerer) -> Self {
        Self {
            lowerer,
            resolved: HashMap::new(),
            visiting: HashSet::new(),
        }
    }

    fn build(mut self) -> Result<hir::HirNominalIdentities, PersistentNominalIdentityError> {
        let struct_ids = self
            .lowerer
            .structs
            .iter()
            .map(|(id, _)| Owner::Struct(id))
            .collect::<Vec<_>>();
        let enum_ids = self
            .lowerer
            .enums
            .iter()
            .map(|(id, _)| Owner::Enum(id))
            .collect::<Vec<_>>();
        let class_ids = self
            .lowerer
            .classes
            .iter()
            .map(|(id, _)| Owner::Class(id))
            .collect::<Vec<_>>();
        let interface_ids = self
            .lowerer
            .interfaces
            .iter()
            .map(|(id, _)| Owner::Interface(id))
            .collect::<Vec<_>>();
        let object_ids = self
            .lowerer
            .objects
            .iter()
            .map(|(id, _)| Owner::Object(id))
            .collect::<Vec<_>>();

        let structs = self.resolve_all(struct_ids)?;
        let enums = self.resolve_all(enum_ids)?;
        let classes = self.resolve_all(class_ids)?;
        let interfaces = self.resolve_all(interface_ids)?;
        let objects = self.resolve_all(object_ids)?;

        hir::HirNominalIdentities::checked(
            &self.lowerer.structs,
            structs,
            &self.lowerer.enums,
            enums,
            &self.lowerer.classes,
            classes,
            &self.lowerer.interfaces,
            interfaces,
            &self.lowerer.objects,
            objects,
        )
        .map_err(|error| self.failure_for_table(error))
    }

    fn resolve_all(
        &mut self,
        owners: Vec<Owner>,
    ) -> Result<Vec<hir::HirNominalIdentity>, PersistentNominalIdentityError> {
        owners
            .into_iter()
            .map(|owner| self.resolve(owner))
            .collect()
    }

    fn resolve(
        &mut self,
        owner: Owner,
    ) -> Result<hir::HirNominalIdentity, PersistentNominalIdentityError> {
        if let Some(identity) = self.resolved.get(&owner) {
            return Ok(identity.clone());
        }
        if !self.visiting.insert(owner) {
            return Err(self.failure(owner, PersistentNominalIdentityErrorDetail::CyclicOwner));
        }

        let result = if let Owner::Class(class) = owner
            && let Some(&object) = self.lowerer.object_by_backing_class.get(&class)
        {
            let object_identity = self.resolve(Owner::Object(object))?;
            let object_type = object_identity.concrete_type_id().ok_or_else(|| {
                self.failure(owner, PersistentNominalIdentityErrorDetail::GenericObject)
            })?;
            hir::HirNominalIdentity::from_generated_key(GeneratedNominalKey::ObjectBackingClass {
                object: object_type,
            })
            .map_err(|error| {
                self.failure(
                    owner,
                    PersistentNominalIdentityErrorDetail::InvalidIdentity(error),
                )
            })?
        } else {
            self.source_identity(owner)?
        };

        self.visiting.remove(&owner);
        self.resolved.insert(owner, result.clone());
        Ok(result)
    }

    fn source_identity(
        &mut self,
        owner: Owner,
    ) -> Result<hir::HirNominalIdentity, PersistentNominalIdentityError> {
        let declaration = self.declaration(owner);
        let file = self.source_file(owner)?;
        let source = self.lowerer.visibility_file(file).source;
        let package = match self.lowerer.top_level_namespaces.source_namespace(file) {
            TopLevelLookupLayer::CorePrelude => PackagePath::root(),
            TopLevelLookupLayer::CurrentPackage(package) => {
                let segments = self
                    .lowerer
                    .top_level_namespaces
                    .package_segments(package)
                    .into_iter()
                    .map(CanonicalIdentifier::new)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| {
                        self.failure(
                            owner,
                            PersistentNominalIdentityErrorDetail::InvalidName(error),
                        )
                    })?;
                PackagePath::from_segments(segments)
            }
        };
        let owners = if let Some(parent) = declaration.parent {
            let parent = Owner::from_nominal_owner(parent);
            let parent_identity = self.resolve(parent)?;
            let source_parent = parent_identity.source().ok_or_else(|| {
                self.failure(
                    owner,
                    PersistentNominalIdentityErrorDetail::GeneratedDeclarationOwner,
                )
            })?;
            let mut owners = source_parent.declaration().owners().owners().to_vec();
            owners.push(source_parent.definition_owner());
            DefinitionOwnerChain::from_outer_to_inner(owners)
        } else {
            DefinitionOwnerChain::top_level()
        };
        let scope = if declaration.parent.is_none()
            && declaration.access == hir::DeclaredVisibility::Private
        {
            DeclarationScope::SourceScoped(source.clone())
        } else {
            DeclarationScope::ConeWide
        };
        let site =
            SourceDeclarationSite::new(source.cone(), package, owners, scope).map_err(|error| {
                self.failure(
                    owner,
                    PersistentNominalIdentityErrorDetail::InvalidSite(error),
                )
            })?;
        let name = CanonicalIdentifier::new(&declaration.name).map_err(|error| {
            self.failure(
                owner,
                PersistentNominalIdentityErrorDetail::InvalidName(error),
            )
        })?;
        let type_parameter_count =
            u32::try_from(declaration.type_parameter_count).map_err(|_| {
                self.failure(
                    owner,
                    PersistentNominalIdentityErrorDetail::TooManyTypeParameters,
                )
            })?;
        let key = SourceDeclarationKey::nominal(site, name, declaration.kind, type_parameter_count);
        hir::HirNominalIdentity::from_source_declaration(key).map_err(|error| {
            self.failure(
                owner,
                PersistentNominalIdentityErrorDetail::InvalidIdentity(error),
            )
        })
    }

    fn declaration(&self, owner: Owner) -> SourceNominalDeclaration {
        match owner {
            Owner::Struct(id) => {
                let declaration = &self.lowerer.structs[id];
                SourceNominalDeclaration {
                    name: declaration.name.clone(),
                    parent: declaration.owner,
                    access: declaration.access.declared,
                    type_parameter_count: declaration.type_params.len(),
                    kind: SourceNominalKind::Struct,
                    span: declaration.span,
                }
            }
            Owner::Enum(id) => {
                let declaration = &self.lowerer.enums[id];
                SourceNominalDeclaration {
                    name: declaration.name.clone(),
                    parent: declaration.owner,
                    access: declaration.access.declared,
                    type_parameter_count: declaration.type_params.len(),
                    kind: SourceNominalKind::Enum,
                    span: declaration.span,
                }
            }
            Owner::Class(id) => {
                let declaration = &self.lowerer.classes[id];
                SourceNominalDeclaration {
                    name: declaration.name.clone(),
                    parent: declaration.owner,
                    access: declaration.access.declared,
                    type_parameter_count: declaration.type_params.len(),
                    kind: SourceNominalKind::Class,
                    span: declaration.span,
                }
            }
            Owner::Interface(id) => {
                let declaration = &self.lowerer.interfaces[id];
                SourceNominalDeclaration {
                    name: declaration.name.clone(),
                    parent: declaration.owner,
                    access: declaration.access.declared,
                    type_parameter_count: declaration.type_params.len(),
                    kind: SourceNominalKind::Interface,
                    span: declaration.span,
                }
            }
            Owner::Object(id) => {
                let declaration = &self.lowerer.objects[id];
                SourceNominalDeclaration {
                    name: declaration.name.clone(),
                    parent: declaration.owner,
                    access: declaration.access.declared,
                    type_parameter_count: 0,
                    kind: SourceNominalKind::Object,
                    span: declaration.span,
                }
            }
        }
    }

    fn source_file(&self, owner: Owner) -> Result<usize, PersistentNominalIdentityError> {
        let file = match owner {
            Owner::Struct(id) => self.lowerer.struct_files.get(&id),
            Owner::Enum(id) => self.lowerer.enum_files.get(&id),
            Owner::Class(id) => self.lowerer.class_files.get(&id),
            Owner::Interface(id) => self.lowerer.interface_files.get(&id),
            Owner::Object(id) => self.lowerer.object_files.get(&id),
        };
        file.copied().ok_or_else(|| {
            self.failure(
                owner,
                PersistentNominalIdentityErrorDetail::MissingSourceFile,
            )
        })
    }

    fn failure(
        &self,
        owner: Owner,
        detail: PersistentNominalIdentityErrorDetail,
    ) -> PersistentNominalIdentityError {
        let declaration = self.declaration(owner);
        let file = match owner {
            Owner::Struct(id) => self.lowerer.struct_files.get(&id),
            Owner::Enum(id) => self.lowerer.enum_files.get(&id),
            Owner::Class(id) => self.lowerer.class_files.get(&id),
            Owner::Interface(id) => self.lowerer.interface_files.get(&id),
            Owner::Object(id) => self.lowerer.object_files.get(&id),
        }
        .copied()
        .unwrap_or(0);
        PersistentNominalIdentityError {
            file,
            span: declaration.span,
            detail,
        }
    }

    fn failure_for_table(
        &self,
        error: hir::HirNominalIdentityTableError,
    ) -> PersistentNominalIdentityError {
        PersistentNominalIdentityError {
            file: 0,
            span: Span { start: 0, end: 0 },
            detail: PersistentNominalIdentityErrorDetail::MisalignedTable(error),
        }
    }
}

struct SourceNominalDeclaration {
    name: String,
    parent: Option<hir::NominalOwner>,
    access: hir::DeclaredVisibility,
    type_parameter_count: usize,
    kind: SourceNominalKind,
    span: Span,
}

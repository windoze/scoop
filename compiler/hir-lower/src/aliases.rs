//! Resolver-only transparent alias graph and Export HIR declarations.

use la_arena::Idx;
use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, NominalTarget, Type};

mod applications;
mod constructors;
mod declarations;
mod forwarding;
mod inference;
mod publication;

pub(crate) type SourceTypeAliasId = Idx<SourceTypeAlias>;

#[derive(Debug, Clone)]
pub(crate) enum AliasOrigin {
    Source(SourceTypeAliasId),
    Imported(Box<hir::DirectImportedTargetBinding>),
}

#[derive(Debug, Clone)]
pub(crate) struct AliasTemplate {
    pub(crate) origin: AliasOrigin,
    pub(crate) name: ast::Ident,
    pub(crate) host: Option<hir::TypeId>,
    pub(crate) arguments: Option<Vec<hir::TypeId>>,
    pub(crate) parameters: Vec<hir::TypeParamDecl>,
    pub(crate) outer_parameter_count: usize,
    pub(crate) target: hir::TypeId,
}

impl AliasTemplate {
    pub(crate) fn parameterized(&self) -> bool {
        self.parameters.len() > self.outer_parameter_count
    }
}

#[derive(Debug, Clone)]
pub(crate) struct SourceTypeAlias {
    name: String,
    target: ast::TypeRef,
    syntax_parameters: Vec<ast::TypeParamDecl>,
    where_clause: Option<ast::WhereClause>,
    pub(crate) parameters: Vec<hir::TypeParamDecl>,
    outer_parameter_count: usize,
    pub(crate) owner: Option<crate::Owner>,
    scope: Option<crate::Scopes>,
    pub(crate) access: hir::DeclarationAccess,
    pub(crate) origin: hir::DefinitionOrigin,
    file: usize,
    resolution: TypeAliasResolution,
}

#[derive(Debug, Clone, Copy)]
enum TypeAliasResolution {
    Unresolved,
    Resolving,
    Resolved(ResolvedTypeAliasTarget),
    Published {
        target: hir::TypeId,
        declaration: hir::ExportTypeAliasId,
    },
    Failed,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedTypeAliasTarget {
    expanded: hir::TypeId,
    source: ResolvedTypeAliasSource,
}

#[derive(Debug, Clone, Copy)]
enum ResolvedTypeAliasSource {
    Expanded,
    Alias(SourceTypeAliasId),
    ImportedAlias(scoop_identity::PersistentTypeAliasId),
}

impl Lowerer {
    pub(crate) fn source_type_alias_is_accessible(&self, id: SourceTypeAliasId) -> bool {
        self.access_domain_allows(&self.source_type_aliases[id].access.lookup.0)
    }

    pub(crate) fn source_type_alias_named(&self, name: &str) -> Option<SourceTypeAliasId> {
        match self.top_level_type_target_for_reference(name)? {
            crate::namespace::TopLevelTypeTarget::Alias(alias) => Some(alias),
            crate::namespace::TopLevelTypeTarget::Nominal(_)
            | crate::namespace::TopLevelTypeTarget::Annotation(_) => None,
        }
    }

    pub(crate) fn resolve_type_alias_reference(
        &mut self,
        name: &ast::Ident,
        supplied_type_arguments: bool,
    ) -> Option<hir::TypeId> {
        match self.resolve_type_lookup(name).ok()?? {
            crate::imports::lookup::TypeLookupTarget::Current(
                crate::namespace::TopLevelTypeTarget::Alias(id),
            ) => self.resolve_type_alias_id_reference(id, name, supplied_type_arguments),
            crate::imports::lookup::TypeLookupTarget::Dependency(binding) => self
                .resolve_imported_dependency_type_target(&binding, name, supplied_type_arguments),
            crate::imports::lookup::TypeLookupTarget::Current(
                crate::namespace::TopLevelTypeTarget::Nominal(_)
                | crate::namespace::TopLevelTypeTarget::Annotation(_),
            ) => None,
        }
    }

    pub(crate) fn resolve_type_alias_id_reference(
        &mut self,
        id: SourceTypeAliasId,
        name: &ast::Ident,
        supplied_type_arguments: bool,
    ) -> Option<hir::TypeId> {
        if supplied_type_arguments {
            self.error(
                name.span,
                format!("typealias `{}` is not generic", name.text),
            );
            return None;
        }
        self.apply_source_type_alias(id, name, &[])
    }

    pub(crate) fn resolve_type_alias(
        &mut self,
        id: SourceTypeAliasId,
        reference_span: ast::Span,
    ) -> Option<hir::TypeId> {
        match self.source_type_aliases[id].resolution {
            TypeAliasResolution::Resolved(ResolvedTypeAliasTarget {
                expanded: target, ..
            })
            | TypeAliasResolution::Published { target, .. } => return Some(target),
            TypeAliasResolution::Failed => return None,
            TypeAliasResolution::Resolving => {
                let cycle_start = self
                    .type_alias_resolution_stack
                    .iter()
                    .position(|candidate| *candidate == id)
                    .unwrap_or(0);
                let mut names = self.type_alias_resolution_stack[cycle_start..]
                    .iter()
                    .map(|candidate| self.source_type_aliases[*candidate].name.clone())
                    .collect::<Vec<_>>();
                names.push(self.source_type_aliases[id].name.clone());
                self.error(
                    reference_span,
                    format!("typealias cycle: {}", names.join(" -> ")),
                );
                self.source_type_aliases[id].resolution = TypeAliasResolution::Failed;
                return None;
            }
            TypeAliasResolution::Unresolved => {}
        }

        let alias = self.source_type_aliases[id].clone();
        self.source_type_aliases[id].resolution = TypeAliasResolution::Resolving;
        self.type_alias_resolution_stack.push(id);

        let previous_file = self.current_file;
        let previous_owner = self.current_owner;
        let previous_params = std::mem::take(&mut self.type_params_in_scope);
        let previous_scope = std::mem::replace(
            &mut self.scopes,
            alias.scope.clone().unwrap_or_else(crate::Scopes::new),
        );
        self.current_file = alias.file;
        self.current_owner = alias.owner;
        let mut parameters = alias.parameters.clone();
        if alias.scope.is_none()
            && let Some(crate::Owner::Object(owner)) = alias.owner
        {
            parameters = self.owner_type_params(crate::Owner::Object(owner));
            parameters.extend(
                alias.parameters[alias.outer_parameter_count..]
                    .iter()
                    .cloned(),
            );
        }
        let parameters = self.resolve_type_parameter_constraints(
            parameters,
            alias.outer_parameter_count,
            &alias.syntax_parameters,
            alias.where_clause.as_ref(),
            "typealias",
        );
        self.source_type_aliases[id].parameters = parameters.clone();
        self.type_params_in_scope = parameters;
        let source = if alias.parameters.is_empty() && alias.scope.is_none() {
            self.resolve_type_alias_source(&alias.target)
        } else {
            ResolvedTypeAliasSource::Expanded
        };
        let target = self.resolve_type_ref(&alias.target);
        self.current_file = previous_file;
        self.current_owner = previous_owner;
        self.type_params_in_scope = previous_params;
        self.scopes = previous_scope;

        let popped = self
            .type_alias_resolution_stack
            .pop()
            .expect("an alias being resolved owns one stack entry");
        debug_assert_eq!(popped, id);
        match target {
            Some(target) => {
                self.source_type_aliases[id].resolution =
                    TypeAliasResolution::Resolved(ResolvedTypeAliasTarget {
                        expanded: target,
                        source,
                    });
                Some(target)
            }
            None => {
                self.source_type_aliases[id].resolution = TypeAliasResolution::Failed;
                None
            }
        }
    }

    /// Classifies only the source target's outermost type reference. Nested
    /// alias uses remain represented by the fully expanded HIR `TypeId`.
    fn resolve_type_alias_source(&self, target: &ast::TypeRef) -> ResolvedTypeAliasSource {
        let alias = match &target.kind {
            ast::TypeRefKind::Named(name) => match self.lookup_type(&name.text) {
                crate::imports::lookup::LookupResult::Unique(candidate) => {
                    Self::classify_type_alias_source_target(candidate.target)
                }
                crate::imports::lookup::LookupResult::Missing
                | crate::imports::lookup::LookupResult::Ambiguous { .. }
                | crate::imports::lookup::LookupResult::Inaccessible(_) => None,
            },
            ast::TypeRefKind::Qualified { path, arguments } if arguments.is_empty() => {
                if let Some(package) = self.qualified_package_prefix(path)
                    && package.consumed() + 1 == path.len()
                {
                    let binding = &path[package.consumed()];
                    match self.lookup_package_type(&package, &binding.text) {
                        crate::imports::lookup::LookupResult::Unique(candidate) => {
                            Self::classify_type_alias_source_target(candidate.target)
                        }
                        crate::imports::lookup::LookupResult::Missing
                        | crate::imports::lookup::LookupResult::Ambiguous { .. }
                        | crate::imports::lookup::LookupResult::Inaccessible(_) => None,
                    }
                } else {
                    None
                }
            }
            ast::TypeRefKind::Generic(_, _)
            | ast::TypeRefKind::AppliedMember { .. }
            | ast::TypeRefKind::Qualified { .. }
            | ast::TypeRefKind::Tuple(_)
            | ast::TypeRefKind::Unit
            | ast::TypeRefKind::Function(_)
            | ast::TypeRefKind::Nullable(_) => None,
        };
        alias.unwrap_or(ResolvedTypeAliasSource::Expanded)
    }

    fn classify_type_alias_source_target(
        target: crate::imports::lookup::TypeLookupTarget,
    ) -> Option<ResolvedTypeAliasSource> {
        match target {
            crate::imports::lookup::TypeLookupTarget::Current(
                crate::namespace::TopLevelTypeTarget::Alias(alias),
            ) => Some(ResolvedTypeAliasSource::Alias(alias)),
            crate::imports::lookup::TypeLookupTarget::Dependency(binding) => {
                match binding.target() {
                    hir::ImportedTarget::TypeAlias(alias) => {
                        Some(ResolvedTypeAliasSource::ImportedAlias(alias.persistent()))
                    }
                    hir::ImportedTarget::Type(_)
                    | hir::ImportedTarget::GenericType(_)
                    | hir::ImportedTarget::Annotation(_) => None,
                    _ => unreachable!("type lookup returns only dependency type targets"),
                }
            }
            crate::imports::lookup::TypeLookupTarget::Current(
                crate::namespace::TopLevelTypeTarget::Nominal(_)
                | crate::namespace::TopLevelTypeTarget::Annotation(_),
            ) => None,
        }
    }

    pub(crate) fn resolve_all_type_aliases(&mut self) {
        let aliases = self
            .source_type_aliases
            .iter()
            .map(|(id, alias)| (id, alias.target.span))
            .collect::<Vec<_>>();
        for (id, span) in aliases {
            self.resolve_type_alias(id, span);
        }
    }

    pub(crate) fn resolved_type_alias_target(&self, id: SourceTypeAliasId) -> Option<hir::TypeId> {
        match self.source_type_aliases[id].resolution {
            TypeAliasResolution::Resolved(ResolvedTypeAliasTarget {
                expanded: target, ..
            })
            | TypeAliasResolution::Published { target, .. } => Some(target),
            TypeAliasResolution::Unresolved
            | TypeAliasResolution::Resolving
            | TypeAliasResolution::Failed => None,
        }
    }

    pub(crate) fn published_type_alias(
        &self,
        id: SourceTypeAliasId,
    ) -> Option<hir::ExportTypeAliasId> {
        match self.source_type_aliases[id].resolution {
            TypeAliasResolution::Published { declaration, .. } => Some(declaration),
            TypeAliasResolution::Unresolved
            | TypeAliasResolution::Resolving
            | TypeAliasResolution::Resolved(_)
            | TypeAliasResolution::Failed => None,
        }
    }

    pub(crate) fn nominal_target_for_type(&self, ty: hir::TypeId) -> Option<NominalTarget> {
        match self.types[ty] {
            Type::Unit | Type::Integer(_) | Type::Boolean | Type::String => {
                let kind = match self.types[ty] {
                    Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
                    Type::Unit => hir::IntrinsicTypeKind::Unit,
                    Type::Boolean => hir::IntrinsicTypeKind::Boolean,
                    Type::String => hir::IntrinsicTypeKind::String,
                    _ => unreachable!("the outer match selected an intrinsic primitive type"),
                };
                let &(owner, _) = self.intrinsic_type_owners.get(&kind)?;
                Some(match owner {
                    crate::IntrinsicTypeOwner::Struct(owner) => NominalTarget::Struct(owner),
                    crate::IntrinsicTypeOwner::Class(owner) => NominalTarget::Class(owner),
                })
            }
            Type::Struct(application) => self
                .source_struct_id(self.struct_applications[application].template)
                .map(NominalTarget::Struct),
            Type::Enum(application) => self
                .source_enum_id(self.enum_applications[application].template)
                .map(NominalTarget::Enum),
            Type::Class(application) => {
                let class = self.source_class_id(self.class_applications[application].template)?;
                Some(
                    self.object_by_backing_class
                        .get(&class)
                        .copied()
                        .map_or(NominalTarget::Class(class), NominalTarget::Object),
                )
            }
            Type::Interface(application) => self
                .source_interface_id(self.interface_applications[application].template)
                .map(NominalTarget::Interface),
            Type::Ptr(_) => self.ffi_ptr.map(NominalTarget::Struct),
            Type::FunPtr(_) => self.ffi_fun_ptr.map(NominalTarget::Struct),
            Type::Any | Type::Tuple(_) | Type::Function(_) | Type::Param(_) => None,
        }
    }
}

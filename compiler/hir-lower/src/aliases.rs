//! Resolver-only transparent alias graph and Export HIR declarations.

use std::collections::HashSet;

use la_arena::Idx;
use scoop_ast as ast;
use scoop_hir as hir;

use crate::{Lowerer, NominalTarget, Type};

pub(crate) type SourceTypeAliasId = Idx<SourceTypeAlias>;

#[derive(Debug, Clone)]
pub(crate) struct SourceTypeAlias {
    name: String,
    target: ast::TypeRef,
    access: hir::DeclarationAccess,
    origin: hir::DefinitionOrigin,
    file: usize,
    resolution: TypeAliasResolution,
}

impl SourceTypeAlias {
    pub(crate) fn file(&self) -> usize {
        self.file
    }
}

#[derive(Debug, Clone, Copy)]
enum TypeAliasResolution {
    Unresolved,
    Resolving,
    Resolved(hir::TypeId),
    Failed,
}

impl Lowerer {
    pub(crate) fn declare_type_alias(&mut self, declaration: &ast::TypeAliasDecl, file: usize) {
        let name = declaration.name.text.clone();
        if let Some(kind) = self.type_namespace_conflict(None, &name) {
            let message = if kind == "a typealias" {
                format!("duplicate typealias `{name}`")
            } else {
                format!("duplicate type `{name}` (already declared as {kind})")
            };
            self.error(declaration.name.span, message);
            return;
        }
        let access = self.top_level_access(
            declaration.visibility,
            declaration.name.span,
            "typealias",
            file,
        );
        let origin = self.definition_origin(declaration.span);
        let id = self.source_type_aliases.alloc(SourceTypeAlias {
            name: name.clone(),
            target: declaration.target.clone(),
            access,
            origin,
            file,
            resolution: TypeAliasResolution::Unresolved,
        });
        self.source_type_aliases_by_name
            .entry(name)
            .or_default()
            .push(id);
    }

    /// The visible typealias binding for a name: exact imports shadow
    /// the current package, which shadows star imports, which shadow the
    /// core unit. Same-layer duplicates are per-package distinct and
    /// unreachable here through different packages.
    pub(crate) fn source_type_alias_named(&self, name: &str) -> Option<SourceTypeAliasId> {
        // An alias-bound import binds the local name regardless of the
        // alias's source name (spec 12.4.1).
        for binding in self.exact_import_bindings(name) {
            if let hir::ImportedTarget::TypeAlias { alias } = binding.target {
                if let Some((source, _)) = self
                    .export_alias_ids
                    .iter()
                    .find(|(_, export)| **export == alias)
                {
                    return Some(*source);
                }
            }
        }
        let aliases = self.source_type_aliases_by_name.get(name)?;
        for layer in crate::imports::LOOKUP_LAYERS {
            if let Some(id) = aliases
                .iter()
                .copied()
                .find(|id| self.source_alias_lookup_layer(*id) == Some(layer))
            {
                return Some(id);
            }
        }
        None
    }

    pub(crate) fn resolve_type_alias_reference(
        &mut self,
        name: &ast::Ident,
        supplied_type_arguments: bool,
    ) -> Option<hir::TypeId> {
        let id = self.source_type_alias_named(&name.text)?;
        if supplied_type_arguments {
            self.error(
                name.span,
                format!("typealias `{}` is not generic", name.text),
            );
            return None;
        }
        let access = self.source_type_aliases[id].access.lookup.0.clone();
        if !self.access_domain_allows(&access, None) {
            self.error(
                name.span,
                format!(
                    "typealias `{}` is not accessible from this source location",
                    name.text
                ),
            );
            return None;
        }
        self.resolve_type_alias(id, name.span)
    }

    fn resolve_type_alias(
        &mut self,
        id: SourceTypeAliasId,
        reference_span: ast::Span,
    ) -> Option<hir::TypeId> {
        match self.source_type_aliases[id].resolution {
            TypeAliasResolution::Resolved(target) => return Some(target),
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
        self.current_file = alias.file;
        self.current_owner = None;
        let target = self.resolve_type_ref(&alias.target);
        self.current_file = previous_file;
        self.current_owner = previous_owner;
        self.type_params_in_scope = previous_params;

        let popped = self
            .type_alias_resolution_stack
            .pop()
            .expect("an alias being resolved owns one stack entry");
        debug_assert_eq!(popped, id);
        match target {
            Some(target) => {
                self.source_type_aliases[id].resolution = TypeAliasResolution::Resolved(target);
                Some(target)
            }
            None => {
                self.source_type_aliases[id].resolution = TypeAliasResolution::Failed;
                None
            }
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

    pub(crate) fn validate_type_alias_targets(&mut self) {
        let previous_file = self.current_file;
        let aliases = self
            .source_type_aliases
            .iter()
            .map(|(id, alias)| (id, alias.clone()))
            .collect::<Vec<_>>();
        for (id, alias) in aliases {
            let TypeAliasResolution::Resolved(target) = self.source_type_aliases[id].resolution
            else {
                continue;
            };
            self.current_file = alias.file;
            let mut visited = HashSet::new();
            self.validate_type_alias_target_tree(
                target,
                alias.target.span,
                &format!("target of typealias `{}`", alias.name),
                &mut visited,
            );
            let mut access = alias.access;
            access.signature = self.signature_exposure_witnesses(
                &access,
                &[target],
                alias.origin.span,
                &format!("typealias `{}`", alias.name),
            );
            let export_id = self.type_aliases.alloc(hir::TypeAliasDecl {
                name: alias.name,
                access,
                target,
                origin: alias.origin,
            });
            self.export_alias_ids.insert(id, export_id);
        }
        self.current_file = previous_file;
    }

    fn validate_type_alias_target_tree(
        &mut self,
        ty: hir::TypeId,
        span: ast::Span,
        description: &str,
        visited: &mut HashSet<hir::TypeId>,
    ) {
        if !visited.insert(ty) {
            return;
        }
        match self.types[ty].clone() {
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                let parameters = self.structs[application.template].type_params.clone();
                self.check_type_argument_kinds(
                    &parameters,
                    &application.arguments,
                    span,
                    description,
                );
                for argument in application.arguments {
                    self.validate_type_alias_target_tree(argument, span, description, visited);
                }
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                let parameters = self.enums[application.template].type_params.clone();
                self.check_type_argument_kinds(
                    &parameters,
                    &application.arguments,
                    span,
                    description,
                );
                for argument in application.arguments {
                    self.validate_type_alias_target_tree(argument, span, description, visited);
                }
            }
            Type::Class(application) => {
                let application = self.class_applications[application].clone();
                let parameters = self.classes[application.template].type_params.clone();
                self.check_type_argument_kinds(
                    &parameters,
                    &application.arguments,
                    span,
                    description,
                );
                for argument in application.arguments {
                    self.validate_type_alias_target_tree(argument, span, description, visited);
                }
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let parameters = self.interfaces[application.template].type_params.clone();
                self.check_type_argument_kinds(
                    &parameters,
                    &application.arguments,
                    span,
                    description,
                );
                for argument in application.arguments {
                    self.validate_type_alias_target_tree(argument, span, description, visited);
                }
            }
            Type::Tuple(elements) => {
                for element in elements {
                    self.validate_type_alias_target_tree(element, span, description, visited);
                }
            }
            Type::Function(function) | Type::FunPtr(function) => {
                let function = self.function_types[function].clone();
                for parameter in function.parameter_types {
                    self.validate_type_alias_target_tree(parameter, span, description, visited);
                }
                self.validate_type_alias_target_tree(
                    function.return_type,
                    span,
                    description,
                    visited,
                );
            }
            Type::Ptr(pointee) => {
                self.validate_type_alias_target_tree(pointee, span, description, visited);
            }
            Type::Unit
            | Type::Integer(_)
            | Type::Boolean
            | Type::String
            | Type::Any
            | Type::Param(_) => {}
        }
    }

    pub(crate) fn type_alias_target(&self, name: &str) -> Option<hir::TypeId> {
        let id = self.source_type_alias_named(name)?;
        match self.source_type_aliases[id].resolution {
            TypeAliasResolution::Resolved(target) => Some(target),
            TypeAliasResolution::Unresolved
            | TypeAliasResolution::Resolving
            | TypeAliasResolution::Failed => None,
        }
    }

    pub(crate) fn nominal_target_for_type(&self, ty: hir::TypeId) -> Option<NominalTarget> {
        match self.types[ty] {
            Type::Integer(_) | Type::Boolean | Type::String => {
                let kind = match self.types[ty] {
                    Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
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
            Type::Struct(application) => Some(NominalTarget::Struct(
                self.struct_applications[application].template,
            )),
            Type::Enum(application) => Some(NominalTarget::Enum(
                self.enum_applications[application].template,
            )),
            Type::Class(application) => {
                let class = self.class_applications[application].template;
                Some(
                    self.object_by_backing_class
                        .get(&class)
                        .copied()
                        .map_or(NominalTarget::Class(class), NominalTarget::Object),
                )
            }
            Type::Interface(application) => Some(NominalTarget::Interface(
                self.interface_applications[application].template,
            )),
            Type::Ptr(_) => self.ffi_ptr.map(NominalTarget::Struct),
            Type::FunPtr(_) => self.ffi_fun_ptr.map(NominalTarget::Struct),
            Type::Unit | Type::Any | Type::Tuple(_) | Type::Function(_) | Type::Param(_) => None,
        }
    }

    pub(crate) fn type_alias_nominal_target(&self, name: &str) -> Option<NominalTarget> {
        self.nominal_target_for_type(self.type_alias_target(name)?)
    }

    /// Resolve an alias used as a nominal qualifier while retaining focused
    /// access and target-kind diagnostics for non-expression evaluators.
    pub(crate) fn resolve_type_alias_nominal_qualifier(
        &mut self,
        name: &ast::Ident,
    ) -> Result<Option<(hir::TypeId, NominalTarget)>, ()> {
        if self.source_type_alias_named(&name.text).is_none() {
            return Ok(None);
        }
        let Some(target) = self.resolve_type_alias_reference(name, false) else {
            return Err(());
        };
        let Some(nominal) = self.nominal_target_for_type(target) else {
            self.error(
                name.span,
                format!("typealias `{}` does not name a type qualifier", name.text),
            );
            return Err(());
        };
        Ok(Some((target, nominal)))
    }

    pub(crate) fn type_alias_is_accessible(&self, name: &str) -> bool {
        self.source_type_alias_named(name).is_some_and(|id| {
            self.access_domain_allows(&self.source_type_aliases[id].access.lookup.0, None)
        })
    }
}

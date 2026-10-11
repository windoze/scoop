//! Validation and atomic publication of resolved source typealiases.

use std::collections::{HashMap, HashSet};

use scoop_ast as ast;
use scoop_hir as hir;

use super::{
    ResolvedTypeAliasSource, ResolvedTypeAliasTarget, SourceTypeAlias, SourceTypeAliasId,
    TypeAliasResolution,
};
use crate::{Lowerer, Type};

impl Lowerer {
    pub(crate) fn validate_type_alias_targets(&mut self) {
        assert!(
            self.type_aliases.is_empty(),
            "type aliases are published exactly once"
        );
        let previous_file = self.current_file;
        let previous_parameters = self.type_params_in_scope.clone();
        let aliases = self
            .source_type_aliases
            .iter()
            .filter(|(_, alias)| alias.scope.is_none())
            .filter_map(|(id, alias)| {
                let TypeAliasResolution::Resolved(target) = alias.resolution else {
                    return None;
                };
                Some((id, alias.clone(), target))
            })
            .collect::<Vec<_>>();
        let publication_ids = aliases
            .iter()
            .enumerate()
            .map(|(index, (source, _, _))| {
                let raw = u32::try_from(index).expect("the type-alias arena index fits in u32");
                (*source, hir::ExportTypeAliasId::from_raw(raw.into()))
            })
            .collect::<HashMap<_, _>>();

        for (id, alias, target) in aliases {
            self.publish_type_alias(id, alias, target, &publication_ids);
        }
        self.current_file = previous_file;
        self.type_params_in_scope = previous_parameters;
    }

    fn publish_type_alias(
        &mut self,
        id: SourceTypeAliasId,
        alias: SourceTypeAlias,
        target: ResolvedTypeAliasTarget,
        publication_ids: &HashMap<SourceTypeAliasId, hir::ExportTypeAliasId>,
    ) {
        self.current_file = alias.file;
        self.type_params_in_scope = alias.parameters.clone();
        let mut visited = HashSet::new();
        self.validate_type_alias_target_tree(
            target.expanded,
            alias.target.span,
            &format!("target of typealias `{}`", alias.name),
            &mut visited,
        );
        let access = alias.access;
        let exposed = std::iter::once(target.expanded)
            .chain(alias.parameters.iter().flat_map(|parameter| {
                parameter
                    .nominal_bounds_in_source_order()
                    .into_iter()
                    .map(|bound| match bound {
                        hir::NominalBoundRef::Class(bound) => bound.ty,
                        hir::NominalBoundRef::Interface(bound) => bound.ty,
                    })
            }))
            .collect::<Vec<_>>();
        self.check_signature_exposure(
            &access,
            &exposed,
            alias.origin.span,
            &format!("typealias `{}`", alias.name),
        );
        let source_target = match target.source {
            ResolvedTypeAliasSource::Expanded => hir::TypeAliasSourceTarget::Expanded,
            ResolvedTypeAliasSource::Alias(source) => hir::TypeAliasSourceTarget::Alias(
                *publication_ids
                    .get(&source)
                    .expect("a resolved alias edge points to another publishable alias"),
            ),
            ResolvedTypeAliasSource::ImportedAlias(source) => {
                hir::TypeAliasSourceTarget::ImportedAlias(source)
            }
        };
        let expected = publication_ids[&id];
        let declaration = self.type_aliases.alloc(hir::TypeAliasDecl {
            lexical: alias.lexical,
            name: alias.name,
            owner: (!alias.lexical)
                .then_some(alias.owner)
                .flatten()
                .map(crate::Owner::as_nominal_owner),
            type_params: alias.parameters,
            owner_parameter_count: if alias.lexical {
                0
            } else {
                alias.outer_parameter_count
            },
            access,
            target: target.expanded,
            source_target,
            origin: alias.origin,
        });
        assert_eq!(
            declaration, expected,
            "type-alias publication preserves the precomputed source-to-export mapping"
        );
        self.source_type_aliases[id].resolution = TypeAliasResolution::Published {
            target: target.expanded,
            declaration,
        };
    }

    pub(crate) fn publish_lexical_type_alias(&mut self, id: SourceTypeAliasId) {
        let TypeAliasResolution::Resolved(target) = self.source_type_aliases[id].resolution else {
            return;
        };
        if let ResolvedTypeAliasSource::Alias(parent) = target.source {
            self.publish_lexical_type_alias(parent);
        }
        let mut ids = self
            .source_type_aliases
            .iter()
            .filter_map(|(id, alias)| match alias.resolution {
                TypeAliasResolution::Published { declaration, .. } => Some((id, declaration)),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        let declaration = hir::ExportTypeAliasId::from_raw((self.type_aliases.len() as u32).into());
        ids.insert(id, declaration);
        let alias = self.source_type_aliases[id].clone();
        let file = self.current_file;
        let parameters = self.type_params_in_scope.clone();
        self.publish_type_alias(id, alias.clone(), target, &ids);
        self.bind_source_annotations(
            hir::SourceAnnotationTarget::TypeAlias(declaration),
            &alias.annotations,
        );
        self.current_file = file;
        self.type_params_in_scope = parameters;
    }

    pub(crate) fn annotate_type_aliases(&mut self) {
        let aliases = self
            .source_type_aliases
            .iter()
            .filter_map(|(_, alias)| {
                if alias.scope.is_some() {
                    return None;
                }
                let TypeAliasResolution::Published { declaration, .. } = alias.resolution else {
                    return None;
                };
                Some((declaration, alias.clone()))
            })
            .collect::<Vec<_>>();
        for (declaration, alias) in aliases {
            self.current_file = alias.file;
            self.current_owner = alias.owner;
            self.type_params_in_scope = alias.parameters;
            self.bind_source_annotations(
                hir::SourceAnnotationTarget::TypeAlias(declaration),
                &alias.annotations,
            );
        }
    }

    pub(super) fn validate_type_alias_target_tree(
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
                let parameters = self
                    .struct_definition(application.template)
                    .type_params
                    .clone();
                self.check_type_argument_kinds(
                    &parameters,
                    &application.arguments,
                    span,
                    description,
                );
                self.validate_type_alias_arguments(
                    application.arguments,
                    span,
                    description,
                    visited,
                );
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                let parameters = self
                    .enum_definition(application.template)
                    .type_params
                    .clone();
                self.check_type_argument_kinds(
                    &parameters,
                    &application.arguments,
                    span,
                    description,
                );
                self.validate_type_alias_arguments(
                    application.arguments,
                    span,
                    description,
                    visited,
                );
            }
            Type::Class(application) => {
                let application = self.class_applications[application].clone();
                let parameters = self
                    .class_definition(application.template)
                    .type_params
                    .clone();
                self.check_type_argument_kinds(
                    &parameters,
                    &application.arguments,
                    span,
                    description,
                );
                self.validate_type_alias_arguments(
                    application.arguments,
                    span,
                    description,
                    visited,
                );
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let parameters = self
                    .interface_definition(application.template)
                    .type_params
                    .clone();
                self.check_type_argument_kinds(
                    &parameters,
                    &application.arguments,
                    span,
                    description,
                );
                self.validate_type_alias_arguments(
                    application.arguments,
                    span,
                    description,
                    visited,
                );
            }
            Type::Tuple(elements) => {
                self.validate_type_alias_arguments(elements, span, description, visited);
            }
            Type::Function(function) | Type::FunPtr(function) => {
                let function = self.function_types[function].clone();
                self.validate_type_alias_arguments(
                    function.parameters().copied().collect(),
                    span,
                    description,
                    visited,
                );
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

    fn validate_type_alias_arguments(
        &mut self,
        arguments: Vec<hir::TypeId>,
        span: ast::Span,
        description: &str,
        visited: &mut HashSet<hir::TypeId>,
    ) {
        for argument in arguments {
            self.validate_type_alias_target_tree(argument, span, description, visited);
        }
    }
}

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
        let aliases = self
            .source_type_aliases
            .iter()
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
    }

    fn publish_type_alias(
        &mut self,
        id: SourceTypeAliasId,
        alias: SourceTypeAlias,
        target: ResolvedTypeAliasTarget,
        publication_ids: &HashMap<SourceTypeAliasId, hir::ExportTypeAliasId>,
    ) {
        self.current_file = alias.file;
        let mut visited = HashSet::new();
        self.validate_type_alias_target_tree(
            target.expanded,
            alias.target.span,
            &format!("target of typealias `{}`", alias.name),
            &mut visited,
        );
        let mut access = alias.access;
        access.signature = self.signature_exposure_witnesses(
            &access,
            &[target.expanded],
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
        };
        let expected = publication_ids[&id];
        let declaration = self.type_aliases.alloc(hir::TypeAliasDecl {
            name: alias.name,
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
                self.validate_type_alias_arguments(
                    application.arguments,
                    span,
                    description,
                    visited,
                );
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
                self.validate_type_alias_arguments(
                    application.arguments,
                    span,
                    description,
                    visited,
                );
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
                self.validate_type_alias_arguments(
                    application.arguments,
                    span,
                    description,
                    visited,
                );
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
                    function.parameter_types,
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

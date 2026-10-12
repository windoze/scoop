use super::*;
use crate::{Owner, namespace::TopLevelTypeTarget};

impl Lowerer {
    pub(crate) fn declare_type_alias(&mut self, declaration: &ast::TypeAliasDecl, file: usize) {
        self.declare_nonlocal_type_alias(declaration, file, None);
    }

    pub(crate) fn declare_nested_type_alias(
        &mut self,
        declaration: &ast::TypeAliasDecl,
        file: usize,
        owner: Owner,
    ) {
        self.declare_nonlocal_type_alias(declaration, file, Some(owner));
    }

    fn declare_nonlocal_type_alias(
        &mut self,
        declaration: &ast::TypeAliasDecl,
        file: usize,
        owner: Option<Owner>,
    ) {
        let name = declaration.name.text.clone();
        if let Some(kind) = self.type_namespace_conflict(
            owner,
            &name,
            file,
            crate::namespace::is_file_private(declaration.visibility),
        ) {
            let message = if kind == "a typealias" {
                format!("duplicate typealias `{name}`")
            } else {
                format!("duplicate type `{name}` (already declared as {kind})")
            };
            self.error(declaration.name.span, message);
            return;
        }
        let access = if let Some(owner) = owner {
            let access = self.nested_nominal_access(
                declaration.visibility,
                declaration.name.span,
                "typealias",
                owner,
                file,
            );
            hir::DeclarationAccess {
                declared: access.declared,
                lookup: access.lookup,
                slot: None,
            }
        } else {
            self.top_level_access(
                declaration.visibility,
                declaration.name.span,
                "typealias",
                file,
            )
        };
        let outer = match owner {
            Some(Owner::Class(id)) if self.enclosing_class_field(id).is_some() => {
                self.owner_type_params(Owner::Class(id))
            }
            Some(Owner::Object(id)) => self.owner_type_params(Owner::Object(id)),
            _ => Vec::new(),
        };
        let scope = owner
            .and_then(|owner| self.local_type_root(owner))
            .map(|_| self.scopes.nested_callable());
        let id = self.allocate_type_alias(declaration, file, owner, access, outer, scope);
        if let Some(owner) = owner {
            self.nested_aliases_by_owner.insert((owner, name), id);
        } else {
            self.top_level_namespaces.register_type(
                file,
                name,
                TopLevelTypeTarget::Alias(id),
                self.source_type_aliases[id].access.declared == hir::DeclaredVisibility::Private,
            );
        }
    }

    pub(crate) fn declare_local_type_alias(&mut self, declaration: &ast::TypeAliasDecl) {
        if self.scopes.type_declared_here(&declaration.name.text) {
            self.error(
                declaration.name.span,
                format!("duplicate local typealias `{}`", declaration.name.text),
            );
            return;
        }
        let id = self.allocate_type_alias(
            declaration,
            self.current_file,
            self.current_owner,
            self.local_declaration_access(),
            self.type_params_in_scope.clone(),
            Some(self.scopes.clone()),
        );
        if let Some(previous) = self
            .scopes
            .declare_type_alias(declaration.name.text.clone(), id)
        {
            self.scopes
                .declare_type_alias(declaration.name.text.clone(), previous);
            self.error(
                declaration.name.span,
                format!("duplicate local typealias `{}`", declaration.name.text),
            );
            self.source_type_aliases[id].resolution = TypeAliasResolution::Failed;
            return;
        }
        self.source_type_aliases[id].scope = Some(self.scopes.clone());
        self.source_type_aliases[id].lexical = true;
        if let Some(target) = self.resolve_type_alias(id, declaration.target.span) {
            let outer = std::mem::replace(
                &mut self.type_params_in_scope,
                self.source_type_aliases[id].parameters.clone(),
            );
            self.validate_type_alias_target_tree(
                target,
                declaration.target.span,
                &format!("target of typealias `{}`", declaration.name.text),
                &mut std::collections::HashSet::new(),
            );
            self.type_params_in_scope = outer;
            self.publish_lexical_type_alias(id);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn allocate_type_alias(
        &mut self,
        declaration: &ast::TypeAliasDecl,
        file: usize,
        owner: Option<Owner>,
        access: hir::DeclarationAccess,
        mut parameters: Vec<hir::TypeParamDecl>,
        scope: Option<crate::Scopes>,
    ) -> SourceTypeAliasId {
        let outer_parameter_count = parameters.len();
        for parameter in &declaration.type_params {
            if parameters
                .iter()
                .any(|existing| existing.name == parameter.name.text)
            {
                self.error(
                    parameter.span,
                    format!("duplicate type parameter `{}`", parameter.name.text),
                );
                continue;
            }
            let id = self.fresh_type_param(parameters.len());
            parameters.push(crate::lower_type_param_decl(parameter, id));
        }
        self.source_type_aliases.alloc(SourceTypeAlias {
            annotations: declaration.annotations.clone(),
            lexical: false,
            name: declaration.name.text.clone(),
            target: declaration.target.clone(),
            syntax_parameters: declaration.type_params.clone(),
            where_clause: declaration.where_clause.clone(),
            parameters,
            outer_parameter_count,
            owner,
            scope,
            access,
            origin: self.definition_origin(declaration.span),
            file,
            resolution: TypeAliasResolution::Unresolved,
        })
    }

    pub(crate) fn lexical_type_alias(&self, name: &str) -> Option<SourceTypeAliasId> {
        if let Some(alias) = self.scopes.type_alias(name) {
            return Some(alias);
        }
        let mut owner = self.current_owner?;
        loop {
            if let Some(alias) = self.nested_aliases_by_owner.get(&(owner, name.to_owned())) {
                return Some(*alias);
            }
            if self.nested_nominal_target(owner, name).is_some() {
                return None;
            }
            owner = self.nominal_parent(owner)?;
        }
    }
}

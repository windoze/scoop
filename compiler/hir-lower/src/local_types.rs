//! Lexical nominal declarations use the ordinary nominal lowering passes.

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{
    StructuralDefinitionPath, StructuralDefinitionSiteRole, StructuralPathSegment,
};

use crate::{Lowerer, NominalTarget, Owner, scope::Scopes};

mod bindings;
mod capture_scope;
mod captures;
mod completion;
mod declarations;
mod exposure;
mod resolution;

#[derive(Clone)]
pub(crate) struct DeclarationContext {
    pub(crate) path: StructuralDefinitionPath,
    pub(crate) parameters: Vec<hir::TypeParamDecl>,
    anonymous: bool,
}

#[derive(Clone)]
pub(crate) struct LocalType {
    pub(crate) outer_parameter_count: usize,
    pub(crate) scopes: Scopes,
    pub(crate) anonymous: bool,
    available: std::collections::HashMap<crate::CaptureKey, crate::AvailableCapture>,
    receivers: Vec<crate::ReceiverScope>,
    captures: Vec<crate::CaptureKey>,
    facts: crate::expr::SmartCastFacts,
    functions: crate::scope::LocalFunctionScopes,
}

#[derive(Clone)]
pub(crate) struct CaptureScope {
    owner: Owner,
    environment: Option<(hir::Expr, hir::BindingId)>,
}

impl Lowerer {
    pub(crate) fn local_type_parameters(&self) -> Vec<hir::TypeParamDecl> {
        self.local_type_declaration
            .as_ref()
            .map_or_else(Vec::new, |context| context.parameters.clone())
    }

    pub(crate) fn local_nominal_scopes(&self, owner: Owner) -> Option<Scopes> {
        self.local_type_root(owner)
            .map(|root| self.local_types[&root].scopes.clone())
    }

    pub(crate) fn local_nominal_functions(
        &self,
        owner: Owner,
    ) -> Option<crate::scope::LocalFunctionScopes> {
        self.local_type_root(owner)
            .map(|root| self.local_types[&root].functions.clone())
    }

    pub(crate) fn local_definition_scopes(
        &self,
        root: hir::LexicalDefinitionRoot,
    ) -> Option<Scopes> {
        self.local_nominal_scopes(self.local_definition_owner(root)?)
    }

    pub(crate) fn local_definition_owner(&self, root: hir::LexicalDefinitionRoot) -> Option<Owner> {
        Some(match root {
            hir::LexicalDefinitionRoot::Function(id) => self.function_owner.get(&id).copied()?,
            hir::LexicalDefinitionRoot::ClassConstructor(id) => {
                Owner::Class(self.class_constructors[id].owner)
            }
            hir::LexicalDefinitionRoot::StructConstructor(id) => {
                Owner::Struct(self.struct_constructors[id].owner)
            }
            hir::LexicalDefinitionRoot::VariantConstructor(id) => Owner::Enum(id.enumeration()),
        })
    }

    pub(crate) fn local_type_parameter_count(&self, owner: Owner) -> usize {
        self.local_types
            .get(&owner)
            .map_or(0, |local| local.outer_parameter_count)
    }

    pub(crate) fn local_nominal_access(&self) -> hir::NominalAccess {
        let access = self.local_declaration_access();
        hir::NominalAccess {
            declared: access.declared,
            inheritance: hir::InheritanceDomain(access.lookup.0.clone()),
            lookup: access.lookup,
        }
    }

    pub(crate) fn register_nominal_name(
        &mut self,
        owner: Option<Owner>,
        name: &ast::Ident,
        target: NominalTarget,
        file: usize,
        visibility: ast::VisibilitySyntax,
    ) {
        if self
            .local_type_declaration
            .as_ref()
            .is_none_or(|context| !context.anonymous)
        {
            self.observe_type_declaration(
                file,
                name.span,
                crate::editing::types::TypeTarget::Nominal(
                    self.nominal_identity(target.owner()).declaration_id(),
                ),
            );
        }
        if let Some(context) = &self.local_type_declaration {
            if !context.anonymous {
                self.scopes.declare_nominal(name.text.clone(), target);
            }
        } else if let Some(owner) = owner {
            self.nested_nominals_by_owner
                .insert((owner, name.text.clone()), target);
        } else {
            self.top_level_namespaces.register_type(
                file,
                name.text.clone(),
                crate::namespace::TopLevelTypeTarget::Nominal(target),
                crate::namespace::is_file_private(visibility),
            );
        }
    }

    pub(crate) fn lower_anonymous_object(
        &mut self,
        declaration: &ast::AnonymousObjectDecl,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let source = ast::LocalTypeDecl::Class(Box::new(ast::ClassDecl {
            annotations: Vec::new(),
            visibility: ast::VisibilitySyntax::Omitted,
            modifier: ast::ClassModifier::Final,
            inner_span: None,
            name: ast::Ident {
                text: "object".into(),
                span: declaration.span,
            },
            type_params: Vec::new(),
            constructor: ast::ClassConstructorDecl::Omitted,
            supertypes: declaration.supertypes.clone(),
            where_clause: None,
            members: declaration.members.clone(),
            span: declaration.span,
        }));
        let Owner::Class(class) = self.lower_local_type(&source, true)? else {
            unreachable!("an object expression declares a class")
        };
        let candidates = self.classes[class]
            .constructors
            .iter()
            .copied()
            .map(crate::call_resolution::candidates::NominalConstructorSource::Class)
            .collect::<Vec<_>>();
        let resolved = self.resolve_nominal_constructor_overload(
            "object",
            &candidates,
            crate::constructor_resolution::NominalConstructorCall {
                enclosing: None,
                explicit_type_args: &[],
                expected_type_args: None,
                arguments: &[],
                span: declaration.span,
            },
            sink,
        )?;
        self.finish_named_nominal(resolved, declaration.span)
    }
}

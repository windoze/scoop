use super::*;
use crate::aliases::AliasTemplate;
use crate::call_resolution::candidates::{NominalConstructorSource, NominalConstructorView};
use crate::constructor_resolution::NominalConstructorCall;

pub(in crate::expr) enum AliasVariantConstructor {
    Source(NominalConstructorView),
    Imported(hir::ImportedCallableDeclaration),
}

impl Lowerer {
    pub(in crate::expr) fn resolve_alias_variant(
        &mut self,
        receiver: &ast::Expr,
        name: &ast::Ident,
    ) -> Result<Option<(AliasTemplate, u32)>, ()> {
        let Some(alias) = self.expression_alias_template(receiver)? else {
            return Ok(None);
        };
        let Type::Enum(application) = self.types[alias.target] else {
            return Ok(None);
        };
        let definition = self.enum_definition(self.enum_applications[application].template);
        Ok(definition
            .variants
            .iter()
            .position(|variant| variant.name == name.text)
            .map(|index| (alias, index as u32)))
    }

    pub(in crate::expr) fn lower_alias_unit_variant(
        &mut self,
        alias: &AliasTemplate,
        index: u32,
        name: &ast::Ident,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let Type::Enum(application) = self.types[alias.target] else {
            unreachable!("an alias variant has an enum target");
        };
        let template = self.enum_applications[application].template;
        if self.enum_definition(template).variants[index as usize].style != VariantStyle::Unit {
            self.error(
                name.span,
                format!(
                    "variant `{}` takes arguments; use `{}(...)` to construct it",
                    name.text, name.text
                ),
            );
            return None;
        }
        let (ty, _) = self.infer_alias_target(alias, &[], expected, name.span)?;
        let Type::Enum(application) = self.types[ty] else {
            unreachable!("alias substitution preserves the enum target");
        };
        let variant = self.enum_variant_at(application, index);
        self.observe_variant_reference(name.span, variant);
        Some(hir::Expr {
            kind: ExprKind::VariantConstruct {
                variant,
                args: Vec::new(),
            },
            ty,
            span: name.span,
            origin: self.expression_origin(name.span),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::expr) fn lower_alias_variant_construct(
        &mut self,
        alias: &AliasTemplate,
        index: u32,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        match self.alias_variant_constructor(alias, index, name, call.span)? {
            AliasVariantConstructor::Source(view) => {
                let explicit = self.resolve_call_type_args(call.type_args)?;
                let probe = match self.probe_named_alias_constructor(
                    view,
                    alias,
                    NominalConstructorCall {
                        enclosing: None,
                        explicit_type_args: &explicit,
                        expected_type_args: None,
                        arguments: call.args,
                        span: call.span,
                    },
                    expected,
                ) {
                    Ok(probe) => probe,
                    Err(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        return None;
                    }
                };
                let resolved = self.commit_named_nominal(probe, call.span, sink)?;
                self.finish_named_nominal(resolved, call.span)
            }
            AliasVariantConstructor::Imported(candidate) => {
                let source_call = ast::CallExpr {
                    callee: name.clone(),
                    type_args: call.type_args.to_vec(),
                    args: call.args.to_vec(),
                    span: call.span,
                };
                match self.probe_imported_constructor(
                    candidate,
                    &source_call,
                    expected,
                    Some(alias),
                ) {
                    Ok(probe) => self.commit_imported_dependency_callable(probe, sink),
                    Err(failure) => {
                        self.commit_layer_diagnostics(*failure);
                        None
                    }
                }
            }
        }
    }

    pub(in crate::expr) fn alias_variant_constructor(
        &mut self,
        alias: &AliasTemplate,
        index: u32,
        name: &ast::Ident,
        span: Span,
    ) -> Option<AliasVariantConstructor> {
        let Type::Enum(application) = self.types[alias.target] else {
            unreachable!("an alias variant has an enum target");
        };
        let template = self.enum_applications[application].template;
        if self.enum_definition(template).variants[index as usize].style == VariantStyle::Unit {
            self.error(
                span,
                format!(
                    "unit variant `{}` does not take arguments; use `{}` without parentheses",
                    name.text, name.text
                ),
            );
            return None;
        }
        if let Some(crate::Owner::Enum(id)) = self.nominal_owners.get(&template).copied() {
            let target = self
                .find_variant_ref(id, &name.text)
                .expect("the selected alias variant exists");
            return Some(AliasVariantConstructor::Source(
                self.nominal_constructor_view(NominalConstructorSource::Variant(target), span),
            ));
        }
        self.imported_variant_constructor(alias.target, index, name.span)
            .map(AliasVariantConstructor::Imported)
    }
}

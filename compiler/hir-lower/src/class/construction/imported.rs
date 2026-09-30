//! Resolve a dependency initializer on the already allocated derived receiver.

use super::*;
use crate::call_resolution::named::NamedFunctionLikeProbe;

impl Lowerer {
    pub(super) fn lower_imported_base_initialization(
        &mut self,
        source: hir::ClassConstructorId,
        owner: TypeId,
        arguments: &[ast::CallArgument],
        span: ast::Span,
        context: &str,
    ) -> Option<hir::BaseInitialization> {
        let Type::Class(application) = self.types[owner] else {
            unreachable!("dependency base initialization has a class owner")
        };
        let identity = self.class_applications[application].template;
        let name = ast::Ident {
            text: self.nominal_template_name(identity).to_owned(),
            span,
        };
        let candidates = self
            .dependencies
            .as_ref()
            .expect("dependency class retains its declaration catalog")
            .constructor_candidates(identity)
            .map_err(|error| {
                self.error(
                    span,
                    format!("invalid dependency constructor declaration: {error}"),
                )
            })
            .ok()?;
        let resolved = self.with_constructor_expression_context(
            source,
            context,
            self.class_constructors[source].safety,
            |this, sink| {
                let mut probes = Vec::new();
                let mut failures = Vec::new();
                for candidate in candidates {
                    match this.probe_imported_value_constructor(
                        candidate,
                        &name,
                        crate::expr::CallSite {
                            type_args: &[],
                            args: arguments,
                            span,
                        },
                        Some(owner),
                    ) {
                        Ok(probe) => {
                            probes.push(NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)))
                        }
                        Err(failure) => failures.push(failure),
                    }
                }
                if probes.is_empty() {
                    if let Some(failure) = failures.into_iter().next() {
                        *this = *failure;
                    } else {
                        this.error(
                            span,
                            format!("class `{}` has no accessible base constructor", name.text),
                        );
                    }
                    return None;
                }
                let winner = this.select_named_function_like(
                    &name.text,
                    "base constructor",
                    &probes,
                    arguments,
                    span,
                )?;
                let NamedFunctionLikeProbe::ImportedDependency(probe) = probes.swap_remove(winner)
                else {
                    unreachable!("base constructor candidates are dependency declarations")
                };
                this.commit_imported_dependency_callable(*probe, sink)
            },
        )?;
        let (target, args) = match resolved.value.kind {
            hir::ExprKind::ImportedDependencyCall { callee, args, .. } => (
                hir::BaseInitializerTarget::Imported {
                    owner,
                    callable: callee,
                },
                args,
            ),
            hir::ExprKind::ImportedConstructorInit { application, args } => (
                hir::BaseInitializerTarget::ImportedTemplate(application),
                args,
            ),
            _ => unreachable!("a dependency constructor resolves to its actual initializer"),
        };
        Some(hir::BaseInitialization::Super {
            target,
            arguments: hir::ConstructorArguments {
                locals: resolved.locals,
                statements: resolved.statements,
                args,
            },
        })
    }
}

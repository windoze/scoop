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
        let Type::ImportedClass(class) = &self.types[owner] else {
            unreachable!("dependency base initialization has a class owner")
        };
        let identity = class.declaration.owner();
        let name = ast::Ident {
            text: class.declaration.name().to_owned(),
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
        let hir::ExprKind::ImportedDependencyCall { callee, args, .. } = resolved.value.kind else {
            unreachable!("a dependency constructor resolves to its actual initializer")
        };
        Some(hir::BaseInitialization::Super {
            target: hir::BaseInitializerTarget::Imported {
                owner,
                callable: callee,
            },
            arguments: hir::ConstructorArguments {
                locals: resolved.locals,
                statements: resolved.statements,
                args,
            },
        })
    }
}

//! Constructor resolution for both namespace bindings and typed nested declarations.

use super::*;

impl Lowerer {
    pub(super) fn collect_imported_constructor_probes(
        mut self,
        owner: scoop_identity::PersistentTypeId,
        call: &ast::CallExpr,
        expected: Option<TypeId>,
        applicable: &mut Vec<NamedApplicable>,
        failures: &mut Vec<Box<Lowerer>>,
    ) {
        let dependencies = self
            .dependencies
            .as_ref()
            .expect("dependency type lookup retains its declaration catalog");
        let declaration = dependencies
            .nominal(owner)
            .expect("a resolved dependency type retains its declaration");
        if declaration.interface.declaration_details().modality()
            == hir::NominalInheritanceModalityV1::Abstract
        {
            self.error(
                call.span,
                format!(
                    "abstract class `{}` cannot be instantiated",
                    declaration.name()
                ),
            );
            failures.push(Box::new(self));
            return;
        }
        let candidates = match dependencies.constructor_candidates(owner) {
            Ok(candidates) => candidates,
            Err(error) => {
                self.error(
                    call.span,
                    format!("invalid dependency constructor declaration: {error}"),
                );
                failures.push(Box::new(self));
                return;
            }
        };
        if candidates.is_empty() {
            self.error(
                call.span,
                format!(
                    "type `{}` does not name a constructible type",
                    call.callee.text
                ),
            );
            failures.push(Box::new(self));
            return;
        }
        for candidate in candidates {
            match self.probe_imported_constructor(candidate, call, expected) {
                Ok(probe) => applicable.push(NamedApplicable {
                    probe: NamedFunctionLikeProbe::ImportedDependency(Box::new(probe)),
                    commit: NamedFunctionCommit::ImportedDependency,
                }),
                Err(failure) => failures.push(failure),
            }
        }
    }

    pub(in crate::expr) fn lower_imported_nominal_construct(
        &mut self,
        owner: scoop_identity::PersistentTypeId,
        name: &ast::Ident,
        call: CallSite<'_>,
        sink: &mut Vec<hir::Statement>,
        expected: Option<TypeId>,
    ) -> Option<hir::Expr> {
        let call = ast::CallExpr {
            callee: name.clone(),
            type_args: call.type_args.to_vec(),
            args: call.args.to_vec(),
            span: call.span,
        };
        let mut applicable = Vec::new();
        let mut failures = Vec::new();
        self.clone().collect_imported_constructor_probes(
            owner,
            &call,
            expected,
            &mut applicable,
            &mut failures,
        );
        if applicable.is_empty() {
            for failure in failures {
                self.commit_layer_diagnostics(*failure);
            }
            return None;
        }
        let mut probes = applicable
            .into_iter()
            .map(|candidate| candidate.probe)
            .collect::<Vec<_>>();
        let winner = self.select_named_function_like(
            &name.text,
            "constructor",
            &probes,
            &call.args,
            call.span,
        )?;
        let NamedFunctionLikeProbe::ImportedDependency(probe) = probes.swap_remove(winner) else {
            unreachable!("dependency constructors use their actual callable declarations")
        };
        self.commit_imported_dependency_callable(*probe, sink)
    }
}

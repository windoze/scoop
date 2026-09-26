use super::*;

impl Lowerer {
    pub(super) fn check_default_reference_access(
        &mut self,
        call_domain: &hir::CallDomain,
        target_domain: &hir::AccessDomain,
        origin: hir::DefinitionOrigin,
        target_kind: &str,
    ) {
        let direct_ok = self.access_domain_is_subset(&call_domain.direct.0, target_domain);
        let slot_ok = call_domain
            .slot
            .as_ref()
            .is_none_or(|slot| self.access_domain_is_subset(&slot.0, target_domain));
        if !direct_ok || !slot_ok {
            let outer_file = self.current_file;
            self.current_file =
                usize::try_from(origin.file).expect("definition file index does not fit usize");
            self.error(origin.span, format!(
                "default expression references {target_kind} outside the callable's complete call domain"
            ));
            self.current_file = outer_file;
        }
    }

    pub(in crate::defaults) fn check_inherited_default_access(
        &mut self,
        function: hir::FunctionId,
        expression: hir::ExportDefaultExprId,
        arguments: &[hir::TypeId],
    ) {
        let body = &self.export_default_exprs[expression];
        let references = body.references.clone();
        let bindings = body
            .type_parameters
            .iter()
            .copied()
            .zip(arguments.iter().copied())
            .collect::<Vec<_>>();
        let call_domain = self.default_call_domain(hir::ExportParameterOwner::Function(function));
        // The same lowering transaction checked these declaration-bound targets.
        // An override can widen its call domain while retaining the original body.
        for (target_domain, origin, kind) in references
            .callables
            .iter()
            .map(|r| (&r.target_domain, r.origin, "a callable"))
            .chain(
                references
                    .constructors
                    .iter()
                    .map(|r| (&r.target_domain, r.origin, "a constructor")),
            )
            .chain(
                references
                    .globals
                    .iter()
                    .map(|r| (&r.target_domain, r.origin, "a property")),
            )
            .chain(
                references
                    .fields
                    .iter()
                    .map(|r| (&r.target_domain, r.origin, "a field")),
            )
        {
            self.check_default_reference_access(&call_domain, target_domain, origin, kind);
        }
        for reference in references.types {
            let ty = match reference.target {
                hir::ExportDefaultTypeTarget::Type(ty) => self.instantiate_method_ty(ty, &bindings),
                hir::ExportDefaultTypeTarget::LocalFunctionSignature(local) => {
                    let signature = self.instantiate_default_local_signature(local, &bindings);
                    self.function_types[signature].canonical_type
                }
            };
            let domain = self.type_access_domain(ty);
            self.check_default_reference_access(&call_domain, &domain, reference.origin, "a type");
        }
    }
}

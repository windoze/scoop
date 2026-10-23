use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

use super::signature::ImportedCallDeclaration;
use super::{
    ImportedCallableCandidate, ImportedDependencyCallProbe, ImportedMemberReceiver,
    ImportedProbeCall,
};
use crate::Lowerer;
use crate::call_resolution::arguments::ArgumentShapeFailure;
use crate::expr::CallSite;

mod native;
mod receiver;

enum ImportedDependencyCallReceiver {
    Implicit,
    Explicit(ImportedMemberReceiver),
}

impl Lowerer {
    pub(in super::super) fn probe_imported_dependency_callable(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        call: &ast::CallExpr,
        expected: Option<hir::TypeId>,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_dependency_callable_with_receiver(
            binding,
            &call.callee,
            CallSite {
                type_args: &call.type_args,
                args: &call.args,
                span: call.span,
            }
            .into(),
            expected,
            ImportedDependencyCallReceiver::Implicit,
            false,
        )
    }

    pub(in crate::expr) fn probe_imported_dependency_extension_callable(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: CallSite<'_>,
        expected: Option<hir::TypeId>,
        operator_set: bool,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_dependency_callable_with_receiver(
            binding,
            name,
            call.into(),
            expected,
            ImportedDependencyCallReceiver::Explicit(ImportedMemberReceiver::Value(receiver)),
            operator_set,
        )
    }

    pub(in crate::expr) fn probe_imported_delegate_extension_callable(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        receiver: hir::Expr,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_dependency_callable_with_receiver(
            binding,
            name,
            call,
            None,
            ImportedDependencyCallReceiver::Explicit(ImportedMemberReceiver::Value(receiver)),
            false,
        )
    }

    fn probe_imported_dependency_callable_with_receiver(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        expected: Option<hir::TypeId>,
        receiver_source: ImportedDependencyCallReceiver,
        operator_set: bool,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let candidate = self
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .callable_candidate(binding)
            .map_err(|error| {
                let mut state = self.clone();
                state.error(
                    name.span,
                    format!("invalid imported dependency callable: {error}"),
                );
                Box::new(state)
            })?;
        self.probe_imported_callable_candidate(
            ImportedCallableCandidate::Binding(Box::new(candidate)),
            name,
            call,
            expected,
            receiver_source,
            operator_set,
        )
    }

    pub(in crate::expr) fn probe_imported_member_callable(
        &self,
        candidate: hir::ImportedCallableDeclaration,
        receiver: ImportedMemberReceiver,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        expected: Option<hir::TypeId>,
        operator_set: bool,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_callable_candidate(
            ImportedCallableCandidate::Declaration(Box::new(candidate)),
            name,
            call,
            expected,
            ImportedDependencyCallReceiver::Explicit(receiver),
            operator_set,
        )
    }

    pub(in crate::expr) fn probe_imported_constructor(
        &self,
        candidate: hir::ImportedCallableDeclaration,
        call: &ast::CallExpr,
        expected: Option<hir::TypeId>,
        alias: Option<&crate::aliases::AliasTemplate>,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let mut input: ImportedProbeCall<'_> = CallSite {
            type_args: &call.type_args,
            args: &call.args,
            span: call.span,
        }
        .into();
        input.alias = alias;
        self.probe_imported_callable_candidate(
            ImportedCallableCandidate::Declaration(Box::new(candidate)),
            &call.callee,
            input,
            expected,
            ImportedDependencyCallReceiver::Implicit,
            false,
        )
    }

    pub(in crate::expr) fn probe_imported_inner_constructor(
        &self,
        candidate: hir::ImportedCallableDeclaration,
        call: &ast::CallExpr,
        expected: Option<hir::TypeId>,
        outer: hir::Expr,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_callable_candidate(
            ImportedCallableCandidate::Declaration(Box::new(candidate)),
            &call.callee,
            CallSite {
                type_args: &call.type_args,
                args: &call.args,
                span: call.span,
            }
            .into(),
            expected,
            ImportedDependencyCallReceiver::Explicit(ImportedMemberReceiver::Value(outer)),
            false,
        )
    }

    pub(crate) fn probe_imported_value_constructor(
        &self,
        candidate: hir::ImportedCallableDeclaration,
        name: &ast::Ident,
        call: CallSite<'_>,
        expected: Option<hir::TypeId>,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_callable_candidate(
            ImportedCallableCandidate::Declaration(Box::new(candidate)),
            name,
            call.into(),
            expected,
            ImportedDependencyCallReceiver::Implicit,
            false,
        )
    }

    fn probe_imported_callable_candidate(
        &self,
        candidate: ImportedCallableCandidate,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        expected: Option<hir::TypeId>,
        receiver_source: ImportedDependencyCallReceiver,
        operator_set: bool,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let mut state = self.clone_imported_probe();
        let interface = candidate.interface();
        let kind = candidate.description();
        if matches!(
            interface.declaration(),
            scoop_identity::CallableTemplateOrigin::Constructor(_)
        ) && let crate::CoreLoweringAuthority::Imported(core) = &state.core
            && interface.owner()
                == hir::PublicDeclarationOwnerV1::Nominal(hir::SourceNominalId::GenericTemplate(
                    core.foreign_callbacks().callback().persistent(),
                ))
        {
            state.error(
                call.span,
                "`ForeignCallback` values can only be produced by `foreignCallback`".into(),
            );
            return Err(state);
        }
        if matches!(
            interface.declaration(),
            scoop_identity::CallableTemplateOrigin::Constructor(_)
        ) && if state.active_member_requirements.is_empty() {
            !state.imported_callable_is_accessible(interface, None)
        } else {
            interface.declared_visibility() != hir::DeclaredVisibilityV1::Public
        } {
            state.error(
                name.span,
                format!("constructor `{}` is not accessible here", name.text),
            );
            return Err(state);
        }
        let constructor_owner = match (interface.declaration(), interface.owner()) {
            (
                scoop_identity::CallableTemplateOrigin::Constructor(_)
                | scoop_identity::CallableTemplateOrigin::VariantConstructor(_),
                hir::PublicDeclarationOwnerV1::Nominal(
                    scoop_identity::NominalDeclarationOwner::GenericTemplate(owner),
                ),
            ) => Some(owner),
            _ => None,
        };
        let expected_type_arguments = constructor_owner.map_or_else(
            || interface.type_parameters().binders().len(),
            |owner| {
                state
                    .dependencies
                    .as_ref()
                    .expect("constructor candidate has a catalog")
                    .nominal_declaration(hir::SourceNominalId::GenericTemplate(owner))
                    .expect("constructor owner is in the dependency catalog")
                    .interface
                    .type_parameters()
                    .binders()
                    .len()
            },
        );
        let enclosing_count = match interface.owner() {
            hir::PublicDeclarationOwnerV1::Nominal(owner)
                if matches!(
                    interface.declaration(),
                    scoop_identity::CallableTemplateOrigin::Constructor(_)
                ) =>
            {
                state
                    .nominal_enclosing_owner(owner)
                    .and_then(|outer| state.dependencies.as_ref()?.nominal_declaration(outer))
                    .map_or(0, |outer| outer.interface.type_parameters().binders().len())
            }
            _ => 0,
        };
        let expected_type_arguments = call
            .alias
            .map_or(expected_type_arguments - enclosing_count, |alias| {
                alias.parameters.len() - alias.outer_parameter_count
            });
        if !call.type_args.is_empty() && call.type_args.len() != expected_type_arguments {
            state.error(
                name.span,
                format!(
                    "dependency {kind} `{}` expects {expected_type_arguments} type argument(s), found {}",
                    name.text,
                    call.type_args.len()
                ),
            );
            return Err(state);
        }
        let Some(argument_mode) = state.imported_call_argument_mode(&candidate, name, call.span)
        else {
            return Err(state);
        };
        let declaration =
            match state.imported_call_declaration(&candidate, constructor_owner.is_some()) {
                Ok(declaration) => declaration,
                Err(error) => {
                    state.error(call.span, error);
                    return Err(state);
                }
            };
        let argument_map = match call.arguments.map(
            &declaration.signature().value_parameters,
            argument_mode,
            operator_set,
        ) {
            Ok(map) => map,
            Err(error) => {
                state.imported_dependency_shape_error(name, call, error, kind);
                return Err(state);
            }
        };

        let receiver = state.imported_callable_receiver(
            &candidate,
            name,
            call.span,
            receiver_source,
            argument_map.has_vararg(),
        )?;
        match declaration {
            ImportedCallDeclaration::Generic(declaration) => state.probe_imported_generic(
                candidate,
                *declaration,
                name,
                call,
                expected,
                receiver,
                argument_map,
            ),
            ImportedCallDeclaration::Native(signature) => state.probe_imported_native(
                candidate,
                signature,
                name,
                call,
                receiver,
                argument_map,
            ),
        }
    }

    fn clone_imported_probe(&self) -> Box<Self> {
        // Keep the complete candidate state off recursive argument-lowering frames.
        Box::new(self.clone())
    }

    fn imported_dependency_shape_error(
        &mut self,
        name: &ast::Ident,
        call: ImportedProbeCall<'_>,
        error: ArgumentShapeFailure,
        kind: &str,
    ) {
        self.error(
            call.span,
            format!("dependency {kind} `{}` {}", name.text, error.describe()),
        );
    }
}

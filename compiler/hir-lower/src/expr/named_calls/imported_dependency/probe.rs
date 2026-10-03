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
use crate::call_resolution::candidates::ArgumentMode;
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
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        self.probe_imported_value_constructor(
            candidate,
            &call.callee,
            CallSite {
                type_args: &call.type_args,
                args: &call.args,
                span: call.span,
            },
            expected,
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
        let mut state = self.clone();
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
            return Err(Box::new(state));
        }
        if matches!(
            interface.declaration(),
            scoop_identity::CallableTemplateOrigin::Constructor(_)
        ) && !state.imported_callable_is_accessible(interface, None)
        {
            state.error(
                name.span,
                format!("constructor `{}` is not accessible here", name.text),
            );
            return Err(Box::new(state));
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
        if !call.type_args.is_empty() && call.type_args.len() != expected_type_arguments {
            state.error(
                name.span,
                format!(
                    "dependency {kind} `{}` expects {expected_type_arguments} type argument(s), found {}",
                    name.text,
                    call.type_args.len()
                ),
            );
            return Err(Box::new(state));
        }
        let mut argument_mode = ArgumentMode::Mixed;
        if let scoop_identity::CallableTemplateOrigin::VariantConstructor(variant) =
            interface.declaration()
        {
            let hir::PublicDeclarationOwnerV1::Nominal(owner) = interface.owner() else {
                unreachable!("a dependency variant retains its enum owner")
            };
            let enumeration = state
                .dependencies
                .as_ref()
                .and_then(|dependencies| dependencies.nominal_declaration(owner))
                .expect("the variant owner is in the dependency catalog");
            let hir::NominalSourceShapeV1::Enum(shape) = enumeration.interface.source_shape()
            else {
                unreachable!("a dependency variant retains its enum source shape")
            };
            let variant = shape
                .variants()
                .iter()
                .find(|value| value.variant() == variant)
                .expect("the shared enum contains its declared variant");
            if variant.style() == hir::EnumSourceVariantStyleV1::Unit {
                state.error(call.span, format!(
                    "unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses",
                    name.text, enumeration.name(), name.text,
                ));
                return Err(Box::new(state));
            }
            argument_mode = match variant.style() {
                hir::EnumSourceVariantStyleV1::Named => ArgumentMode::NamedOnly,
                hir::EnumSourceVariantStyleV1::Positional => ArgumentMode::PositionalOnly,
                hir::EnumSourceVariantStyleV1::Constructor => ArgumentMode::Mixed,
                hir::EnumSourceVariantStyleV1::Unit => unreachable!("unit variants were rejected"),
            };
        }
        let declaration =
            match state.imported_call_declaration(&candidate, constructor_owner.is_some()) {
                Ok(declaration) => declaration,
                Err(error) => {
                    state.error(call.span, error);
                    return Err(Box::new(state));
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
                return Err(Box::new(state));
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
            ImportedCallDeclaration::Generic(declaration) => Box::new(state)
                .probe_imported_generic(
                    candidate,
                    *declaration,
                    name,
                    call,
                    expected,
                    receiver,
                    argument_map,
                ),
            ImportedCallDeclaration::Native(signature) => Box::new(state).probe_imported_native(
                candidate,
                signature,
                name,
                call,
                receiver,
                argument_map,
            ),
        }
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

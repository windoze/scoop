use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

use super::{
    ImportedCallableCandidate, ImportedDependencyCallProbe, ImportedMemberReceiver,
    ImportedProbeCall,
};
use crate::Lowerer;
use crate::call_resolution::arguments::ArgumentShapeFailure;
use crate::expr::CallSite;

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
            },
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
            call,
            expected,
            ImportedDependencyCallReceiver::Explicit(ImportedMemberReceiver::Value(receiver)),
            operator_set,
        )
    }

    fn probe_imported_dependency_callable_with_receiver(
        &self,
        binding: &hir::DirectImportedTargetBinding,
        name: &ast::Ident,
        call: CallSite<'_>,
        expected: Option<hir::TypeId>,
        receiver_source: ImportedDependencyCallReceiver,
        operator_set: bool,
    ) -> Result<ImportedDependencyCallProbe, Box<Lowerer>> {
        let mut state = self.clone();
        let candidate = match state
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .callable_candidate(binding)
        {
            Ok(candidate) => candidate,
            Err(error) => {
                state.error(
                    name.span,
                    format!("invalid imported dependency callable: {error}"),
                );
                return Err(Box::new(state));
            }
        };
        self.probe_imported_callable_candidate(
            ImportedCallableCandidate::Binding(Box::new(candidate)),
            name,
            call.into(),
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
        ) && !state.imported_callable_is_accessible(interface, None)
        {
            state.error(
                name.span,
                format!("constructor `{}` is not accessible here", name.text),
            );
            return Err(Box::new(state));
        }
        let expected_type_arguments = interface.type_parameters().binders().len();
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
        let Some(source) = candidate.source_interface() else {
            state.error(
                name.span,
                format!(
                    "invalid imported dependency callable `{}`: source interface is missing",
                    name.text
                ),
            );
            return Err(Box::new(state));
        };
        if let scoop_identity::CallableTemplateOrigin::VariantConstructor(variant) =
            interface.declaration()
        {
            let Ok(owner) = state.imported_signature_type(interface.result()) else {
                state.imported_dependency_capability_error(
                    &candidate,
                    false,
                    "dependency enum variant",
                    call.span,
                );
                return Err(Box::new(state));
            };
            let hir::Type::ImportedEnum(enumeration) = &state.types[owner] else {
                unreachable!("a dependency variant retains its actual enum result")
            };
            let variant = enumeration
                .variants
                .iter()
                .find(|value| value.identity == variant)
                .expect("the shared enum contains its declared variant");
            if variant.style == hir::EnumSourceVariantStyleV1::Unit {
                state.error(call.span, format!(
                    "unit variant `{}` of `{}` does not take arguments; use `{}` without parentheses",
                    name.text, enumeration.declaration.name(), name.text,
                ));
                return Err(Box::new(state));
            }
            if let Err(error) = call.arguments.check_variant_style(variant.style) {
                state.imported_dependency_shape_error(name, call, error, kind);
                return Err(Box::new(state));
            }
        }
        let argument_map = match call
            .arguments
            .map(source.parameters().parameters(), operator_set)
        {
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
        if candidate.callable_body().is_some()
            && matches!(
                candidate.interface().owner(),
                hir::PublicDeclarationOwnerV1::TopLevel | hir::PublicDeclarationOwnerV1::Extension
            )
        {
            return state.probe_imported_generic(
                candidate,
                name,
                call,
                expected,
                receiver,
                argument_map,
            );
        }
        if candidate.executable() || matches!(candidate, ImportedCallableCandidate::Declaration(_))
        {
            for signature in interface
                .parameters()
                .parameters()
                .iter()
                .map(|parameter| parameter.value_type())
                .chain(std::iter::once(interface.result()))
            {
                if state.imported_signature_type(signature).is_err() {
                    state.imported_dependency_capability_error(
                        &candidate,
                        argument_map.has_vararg(),
                        "dependency member signature",
                        call.span,
                    );
                    return Err(Box::new(state));
                }
            }
        }
        let parameter_types = interface
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| {
                state
                    .imported_signature_type(parameter.value_type())
                    .unwrap_or(state.any)
            })
            .collect::<Vec<_>>();
        let result_type = state
            .imported_signature_type(interface.result())
            .unwrap_or(state.any);
        if candidate.executable()
            && let Some(expected) = expected
            && !state.is_subtype(result_type, expected)
        {
            state.error(
                call.span,
                format!(
                    "dependency {kind} `{}` returns {}, which is not compatible with expected {}",
                    name.text,
                    state.type_name(result_type),
                    state.type_name(expected)
                ),
            );
            return Err(Box::new(state));
        }

        let mut source_args = Vec::with_capacity(call.arguments.len());
        let mut argument_sinks = Vec::with_capacity(call.arguments.len());
        let mut integer_arguments = Vec::with_capacity(call.arguments.len());
        for (index, signature) in argument_map.source_parameters().iter().enumerate() {
            let parameter = state.imported_signature_type(signature).ok();
            let mut argument_sink = Vec::new();
            let Some(value) =
                call.arguments
                    .lower(index, &mut state, &mut argument_sink, parameter)
            else {
                return Err(Box::new(state));
            };
            if let Some(parameter) = parameter
                && !state.is_subtype(value.ty, parameter)
            {
                state.error(
                    call.arguments.span(index),
                    format!(
                        "dependency {kind} argument must be of type {}, found {}",
                        state.type_name(parameter),
                        state.type_name(value.ty)
                    ),
                );
                return Err(Box::new(state));
            }
            integer_arguments.push(match state.types[value.ty] {
                hir::Type::Integer(kind) => Some(kind),
                _ => None,
            });
            source_args.push(match parameter {
                Some(parameter) => state.adapt_to(value, parameter),
                None => value,
            });
            argument_sinks.push(argument_sink);
        }
        if !candidate.executable()
            || (matches!(
                receiver,
                super::ImportedCallReceiver::Member {
                    value: ImportedMemberReceiver::LiteralSubject(_),
                    ..
                }
            ) && candidate.integer_equality_kind().is_none())
        {
            state.imported_dependency_capability_error(
                &candidate,
                argument_map.has_vararg(),
                "dependency callable",
                call.span,
            );
            return Err(Box::new(state));
        }
        let default_plan = match state.prepare_imported_defaults(&candidate, &argument_map) {
            Ok(plan) => plan,
            Err(error) => {
                state.error(call.span, error.to_string());
                return Err(Box::new(state));
            }
        };

        Ok(ImportedDependencyCallProbe {
            implementation: super::ImportedCallImplementation::Native,
            declaration_file: state.current_file,
            declaration_span: name.span,
            state: Box::new(state),
            candidate,
            receiver,
            source_args,
            argument_sinks,
            argument_map,
            default_plan,
            parameter_types,
            result_type,
            integer_arguments,
            call_span: call.span,
        })
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

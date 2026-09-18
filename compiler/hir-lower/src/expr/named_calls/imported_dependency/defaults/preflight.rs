use std::collections::{BTreeMap, BTreeSet};

use scoop_hir as hir;
use scoop_identity::{LocalValueSelector, SignatureTypeKey};

use super::plan::{
    ImportedDefaultPlanError, PreparedImportedDefault, PreparedImportedDefaultCallable,
};
use crate::Lowerer;
use crate::imported_capabilities::{ImportedCapabilityRequirement, callable_requirement};
use crate::imported_core::ImportedSignatureTypeError;

mod statements;

impl Lowerer {
    pub(super) fn prepare_imported_default(
        &mut self,
        owner: &hir::ImportedDependencyCallableCandidate,
        template: hir::ExportDefaultTemplateV1,
    ) -> Result<PreparedImportedDefault, ImportedDefaultPlanError> {
        if !template.type_parameters().is_empty() {
            return Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Generic,
                operation: "dependency default type substitution",
            });
        }
        self.imported_default_core_type(template.result())?;

        let mut locals = BTreeSet::new();
        for local in template.locals().records() {
            self.imported_default_core_type(local.value_type())?;
            locals.insert(local.selector().clone());
        }
        let mut callables = BTreeMap::new();
        self.preflight_imported_default_statements(
            owner,
            &template,
            template.body().statements(),
            &locals,
            &mut callables,
            0,
        )?;
        self.preflight_imported_default_expression(
            owner,
            &template,
            template.body().value(),
            &locals,
            &mut callables,
        )?;
        Ok(PreparedImportedDefault {
            template,
            callables,
        })
    }

    fn preflight_imported_default_expression(
        &mut self,
        owner: &hir::ImportedDependencyCallableCandidate,
        template: &hir::ExportDefaultTemplateV1,
        expression: &hir::DefaultExpressionV1,
        locals: &BTreeSet<LocalValueSelector>,
        callables: &mut BTreeMap<hir::DefaultCallableRefV1, PreparedImportedDefaultCallable>,
    ) -> Result<(), ImportedDefaultPlanError> {
        self.imported_default_core_type(expression.result_type())?;
        use hir::DefaultExpressionKindV1 as Kind;
        match expression.kind() {
            Kind::StringLiteral {
                owner: hir::DefaultStringOwnerV1::CurrentInstantiation,
                ..
            }
            | Kind::IntegerLiteral(_)
            | Kind::BooleanLiteral(_)
            | Kind::UnitLiteral => Ok(()),
            Kind::StringLiteral {
                owner: hir::DefaultStringOwnerV1::Property(_),
                ..
            } => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Layout,
                operation: "dependency-owned string constant",
            }),
            Kind::Local(local) if locals.contains(local) => Ok(()),
            Kind::Local(local) => Err(ImportedDefaultPlanError::UnknownLocal(local.clone())),
            Kind::Call { callee, arguments } => {
                self.prepare_imported_default_call(template, callee, callables)?;
                self.preflight_imported_default_expressions(
                    owner, template, arguments, locals, callables,
                )
            }
            Kind::PrimitiveBinary { lhs, rhs, .. } | Kind::Binary { lhs, rhs, .. } => {
                self.preflight_imported_default_expression(
                    owner, template, lhs, locals, callables,
                )?;
                self.preflight_imported_default_expression(owner, template, rhs, locals, callables)
            }
            Kind::PrimitiveUnary { operand, .. } | Kind::Unary { operand, .. } => self
                .preflight_imported_default_expression(owner, template, operand, locals, callables),
            Kind::IntegerOperation {
                operation,
                arguments,
            } => {
                let target = match operation {
                    hir::DefaultIntegerOperationV1::NoGc { target, .. }
                    | hir::DefaultIntegerOperationV1::Managed { target, .. } => target,
                };
                self.prepare_imported_default_call(template, target, callables)?;
                match arguments {
                    hir::DefaultIntegerArgumentsV1::Unary(operand) => self
                        .preflight_imported_default_expression(
                            owner, template, operand, locals, callables,
                        ),
                    hir::DefaultIntegerArgumentsV1::Binary { lhs, rhs } => {
                        self.preflight_imported_default_expression(
                            owner, template, lhs, locals, callables,
                        )?;
                        self.preflight_imported_default_expression(
                            owner, template, rhs, locals, callables,
                        )
                    }
                }
            }
            Kind::IntegerConversion {
                target, operand, ..
            } => {
                self.prepare_imported_default_call(template, target, callables)?;
                self.preflight_imported_default_expression(
                    owner, template, operand, locals, callables,
                )
            }
            Kind::MethodCall { .. } | Kind::DirectSuperMethodCall { .. } => {
                Err(ImportedDefaultPlanError::Requires {
                    requirement: ImportedCapabilityRequirement::Dispatch,
                    operation: "dependency default dispatch",
                })
            }
            Kind::Lambda(_)
            | Kind::AnonymousFunction(_)
            | Kind::CallableReference(_)
            | Kind::FunctionCoercion { .. }
            | Kind::LocalFunctionCall { .. }
            | Kind::CallableCall { .. } => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Generic,
                operation: "dependency default callable value",
            }),
            Kind::PtrFromNonZeroULong(_)
            | Kind::PtrToULong(_)
            | Kind::PtrCast(_)
            | Kind::PtrLoad { .. }
            | Kind::PtrStore { .. }
            | Kind::PtrOffset { .. }
            | Kind::AddressOf(_)
            | Kind::FunctionAddress(_)
            | Kind::ForeignCallbackRegister { .. }
            | Kind::ForeignCallbackOperation { .. } => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Native,
                operation: "dependency default native operation",
            }),
            Kind::TupleLiteral(_)
            | Kind::StructInit { .. }
            | Kind::StructConstruct { .. }
            | Kind::ClassInit { .. }
            | Kind::VariantConstruct { .. }
            | Kind::VariantTest { .. }
            | Kind::VariantPayloadProject { .. }
            | Kind::GlobalRead(_)
            | Kind::SingletonValue(_)
            | Kind::SizeOf(_)
            | Kind::AlignOf(_)
            | Kind::FieldAccess { .. }
            | Kind::Box(_)
            | Kind::Unbox(_)
            | Kind::IsInstance { .. }
            | Kind::Cast { .. }
            | Kind::ArrayLiteral(_)
            | Kind::ArrayAssembly(_)
            | Kind::Index { .. }
            | Kind::ArraySet { .. }
            | Kind::ArrayLen(_)
            | Kind::ArrayClone(_)
            | Kind::SomeWrap(_)
            | Kind::NoneLiteral
            | Kind::IsSome(_)
            | Kind::Unwrap { .. } => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Layout,
                operation: "dependency default value layout",
            }),
        }
    }

    fn preflight_imported_default_expressions(
        &mut self,
        owner: &hir::ImportedDependencyCallableCandidate,
        template: &hir::ExportDefaultTemplateV1,
        expressions: &[hir::DefaultExpressionV1],
        locals: &BTreeSet<LocalValueSelector>,
        callables: &mut BTreeMap<hir::DefaultCallableRefV1, PreparedImportedDefaultCallable>,
    ) -> Result<(), ImportedDefaultPlanError> {
        for expression in expressions {
            self.preflight_imported_default_expression(
                owner, template, expression, locals, callables,
            )?;
        }
        Ok(())
    }

    fn prepare_imported_default_call(
        &self,
        template: &hir::ExportDefaultTemplateV1,
        callee: &hir::DefaultCallableRefV1,
        callables: &mut BTreeMap<hir::DefaultCallableRefV1, PreparedImportedDefaultCallable>,
    ) -> Result<(), ImportedDefaultPlanError> {
        if callables.contains_key(callee) {
            return Ok(());
        }
        if callee.owner().is_present() {
            return Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Dispatch,
                operation: "dependency default member call",
            });
        }
        if !callee.type_arguments().is_empty()
            || matches!(
                callee.declaration(),
                hir::DefaultCallableDeclarationV1::GenericFunction(_)
            )
        {
            return Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Generic,
                operation: "dependency default generic call",
            });
        }
        let target = hir::ExportDefaultCallableTargetV1::Callable(callee.clone());
        if !template
            .references()
            .callables()
            .iter()
            .any(|reference| reference.target() == &target)
        {
            return Err(ImportedDefaultPlanError::MissingCallableReference(
                callee.clone(),
            ));
        }

        if let Some(reference) = self.imported_core_callable_by_declaration(callee.declaration()) {
            callables.insert(
                callee.clone(),
                PreparedImportedDefaultCallable::Core(reference),
            );
            return Ok(());
        }
        let candidate = self
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .default_callable_candidate(callee.declaration())
            .map_err(|error| ImportedDefaultPlanError::Callable {
                callee: callee.clone(),
                error: error.to_string(),
            })?;
        if candidate.capability().is_none() {
            return Err(ImportedDefaultPlanError::Requires {
                requirement: callable_requirement(&candidate, false),
                operation: "dependency default call",
            });
        }
        callables.insert(
            callee.clone(),
            PreparedImportedDefaultCallable::Dependency(Box::new(candidate)),
        );
        Ok(())
    }

    pub(super) fn imported_default_core_type(
        &mut self,
        signature: &SignatureTypeKey,
    ) -> Result<hir::TypeId, ImportedDefaultPlanError> {
        match self.imported_signature_type(signature) {
            Ok(ty) if matches!(signature, SignatureTypeKey::Nominal(_)) => Ok(ty),
            Ok(_) | Err(ImportedSignatureTypeError::Structural) => {
                Err(ImportedDefaultPlanError::Requires {
                    requirement: ImportedCapabilityRequirement::Layout,
                    operation: "dependency default value type",
                })
            }
            Err(ImportedSignatureTypeError::Generic) => Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Generic,
                operation: "dependency default generic type",
            }),
        }
    }
}

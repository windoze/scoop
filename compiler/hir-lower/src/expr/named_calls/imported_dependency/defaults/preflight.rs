use std::collections::{BTreeMap, BTreeSet};

use scoop_hir as hir;
use scoop_identity::{LocalValueSelector, SignatureTypeKey};

use super::plan::{ImportedDefaultPlanError, PreparedImportedDefault};
use crate::Lowerer;
use crate::imported_capabilities::{ImportedCapabilityRequirement, callable_requirement};
use crate::imported_core::ImportedSignatureTypeError;

mod statements;

impl Lowerer {
    pub(super) fn prepare_imported_default(
        &mut self,
        owner: &dyn hir::ImportedCallableSource,
        template: hir::ExportDefaultTemplateV1,
    ) -> Result<PreparedImportedDefault, ImportedDefaultPlanError> {
        if !template.type_parameters().is_empty() {
            return Err(ImportedDefaultPlanError::Requires {
                requirement: ImportedCapabilityRequirement::Generic,
                operation: "dependency default type substitution",
            });
        }
        self.imported_default_type(template.result())?;

        let mut locals = BTreeSet::new();
        for local in template.locals().records() {
            self.imported_default_type(local.value_type())?;
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
        owner: &dyn hir::ImportedCallableSource,
        template: &hir::ExportDefaultTemplateV1,
        expression: &hir::DefaultExpressionV1,
        locals: &BTreeSet<LocalValueSelector>,
        callables: &mut BTreeMap<
            scoop_identity::CallableTemplateOrigin,
            hir::ImportedCallableDeclaration,
        >,
    ) -> Result<(), ImportedDefaultPlanError> {
        self.imported_default_type(expression.result_type())?;
        use hir::DefaultExpressionKindV1 as Kind;
        match expression.kind() {
            Kind::ReferenceUpcast(operand) | Kind::Box(operand) | Kind::Unbox(operand) => self
                .preflight_imported_default_expression(owner, template, operand, locals, callables),
            Kind::IsInstance {
                operand,
                checked_type,
            }
            | Kind::Cast {
                operand,
                checked_type,
                ..
            } => {
                self.imported_default_type(checked_type)?;
                self.preflight_imported_default_expression(
                    owner, template, operand, locals, callables,
                )
            }
            Kind::StringLiteral {
                owner: hir::DefaultStringOwnerV1::CurrentInstantiation,
                ..
            }
            | Kind::IntegerLiteral(_)
            | Kind::BooleanLiteral(_)
            | Kind::SingletonValue(_)
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
            Kind::TupleLiteral(elements) => self.preflight_imported_default_expressions(
                owner, template, elements, locals, callables,
            ),
            Kind::VariantConstruct { variant, arguments } => {
                self.imported_default_type(variant.owner_type())?;
                self.preflight_imported_default_expressions(
                    owner, template, arguments, locals, callables,
                )
            }
            Kind::Call {
                callee,
                arguments,
                receiver,
            } => {
                receiver
                    .as_ref()
                    .try_map(|ty| self.imported_default_type(ty))?;
                self.prepare_imported_default_call(
                    super::plan::default_callable_origin(callee)?,
                    callables,
                )?;
                self.preflight_imported_default_expressions(
                    owner, template, arguments, locals, callables,
                )
            }
            Kind::StructInit {
                constructor:
                    hir::DefaultConstructorRefV1::Struct {
                        declaration,
                        owner_type,
                    },
                arguments,
            }
            | Kind::ClassInit {
                constructor:
                    hir::DefaultConstructorRefV1::Class {
                        declaration: hir::DefaultClassConstructorIdV1::Source(declaration),
                        owner_type,
                    },
                arguments,
            } => {
                self.imported_default_type(owner_type)?;
                self.prepare_imported_default_call(
                    scoop_identity::CallableTemplateOrigin::Constructor(*declaration),
                    callables,
                )?;
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
            Kind::IntegerOperation { arguments, .. } => match arguments {
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
            },
            Kind::IntegerConversion { operand, .. } => self
                .preflight_imported_default_expression(owner, template, operand, locals, callables),
            Kind::FieldAccess {
                receiver,
                field: hir::DefaultFieldRefV1::Struct { owner_type, .. },
            } => {
                self.imported_default_type(owner_type)?;
                self.preflight_imported_default_expression(
                    owner, template, receiver, locals, callables,
                )
            }
            Kind::FieldAccess {
                receiver,
                field: hir::DefaultFieldRefV1::Tuple { .. },
            } => self.preflight_imported_default_expression(
                owner, template, receiver, locals, callables,
            ),
            Kind::MethodCall {
                receiver,
                callee: hir::DefaultMethodCalleeV1::Callable(callee),
                arguments,
            } => {
                self.prepare_imported_default_call(
                    super::plan::default_callable_origin(callee)?,
                    callables,
                )?;
                self.preflight_imported_default_expression(
                    owner, template, receiver, locals, callables,
                )?;
                self.preflight_imported_default_expressions(
                    owner, template, arguments, locals, callables,
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
            Kind::StructInit { .. }
            | Kind::StructConstruct { .. }
            | Kind::ClassInit { .. }
            | Kind::VariantTest { .. }
            | Kind::VariantPayloadProject { .. }
            | Kind::GlobalRead(_)
            | Kind::SizeOf(_)
            | Kind::AlignOf(_)
            | Kind::FieldAccess { .. }
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
        owner: &dyn hir::ImportedCallableSource,
        template: &hir::ExportDefaultTemplateV1,
        expressions: &[hir::DefaultExpressionV1],
        locals: &BTreeSet<LocalValueSelector>,
        callables: &mut BTreeMap<
            scoop_identity::CallableTemplateOrigin,
            hir::ImportedCallableDeclaration,
        >,
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
        callee: scoop_identity::CallableTemplateOrigin,
        callables: &mut BTreeMap<
            scoop_identity::CallableTemplateOrigin,
            hir::ImportedCallableDeclaration,
        >,
    ) -> Result<(), ImportedDefaultPlanError> {
        if callables.contains_key(&callee) {
            return Ok(());
        }
        let candidate = self
            .dependencies
            .as_ref()
            .expect("ordinary lowering carries a dependency selection plan")
            .callable_declaration(callee)
            .map_err(|error| ImportedDefaultPlanError::Callable {
                callee,
                error: error.to_string(),
            })?;
        if candidate.capability().is_none() {
            return Err(ImportedDefaultPlanError::Requires {
                requirement: callable_requirement(&candidate, false),
                operation: "dependency default call",
            });
        }
        callables.insert(callee, candidate);
        Ok(())
    }

    pub(super) fn imported_default_type(
        &mut self,
        signature: &SignatureTypeKey,
    ) -> Result<hir::TypeId, ImportedDefaultPlanError> {
        match self.imported_signature_type(signature) {
            Ok(ty) => Ok(ty),
            Err(ImportedSignatureTypeError::Structural) => {
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

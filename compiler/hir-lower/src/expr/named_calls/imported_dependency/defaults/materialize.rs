use std::collections::BTreeMap;
use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::LocalValueSelector;

use super::plan::PreparedImportedDefault;
use crate::Lowerer;
use crate::expr::MemberCallKind;
use crate::expr::imported_origins::ImportedDefinitionOriginError;

mod statements;

struct ImportedDefaultContext<'a> {
    owner: &'a dyn hir::ImportedCallableSource,
    prepared: &'a PreparedImportedDefault,
    locals: BTreeMap<LocalValueSelector, hir::Expr>,
    loop_targets: Vec<hir::LoopId>,
    evaluation: hir::EvaluationOrigin,
}

impl Lowerer {
    pub(crate) fn materialize_imported_default(
        &mut self,
        owner: &dyn hir::ImportedCallableSource,
        prepared: &PreparedImportedDefault,
        receiver: Option<&hir::Expr>,
        value_parameters: &[hir::Expr],
        call_span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Result<hir::Expr, ImportedDefaultMaterializationError> {
        let mut locals = BTreeMap::new();
        if let Some(template_receiver) = prepared.template.receiver().receiver() {
            let value = receiver
                .cloned()
                .ok_or(ImportedDefaultMaterializationError::MissingReceiver)?;
            let ty = self
                .imported_default_type(template_receiver.value_type())
                .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;
            let value = self.adapt_to(value, ty);
            locals.insert(template_receiver.local().clone(), value);
        }
        for parameter in prepared.template.value_parameters().parameters() {
            let position = usize::try_from(parameter.position())
                .map_err(|_| ImportedDefaultMaterializationError::ParameterIndexOverflow)?;
            let value = value_parameters.get(position).cloned().ok_or(
                ImportedDefaultMaterializationError::MissingValueParameter {
                    position: parameter.position(),
                },
            )?;
            locals.insert(parameter.local().clone(), value);
        }
        for (index, local) in prepared.template.locals().records().iter().enumerate() {
            if locals.contains_key(local.selector()) {
                continue;
            }
            let ty = self
                .imported_default_type(local.value_type())
                .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;
            let id = self.alloc_synthetic_local(
                format!("$dependency.default.local.{index}"),
                ty,
                local.mutable().into(),
                scoop_identity::SyntheticLocalRole::DefaultValue,
            );
            locals.insert(
                local.selector().clone(),
                hir::Expr {
                    kind: hir::ExprKind::Local(id),
                    ty,
                    span: call_span,
                    origin: self.expression_origin(call_span),
                },
            );
        }
        let mut context = ImportedDefaultContext {
            owner,
            prepared,
            locals,
            loop_targets: Vec::new(),
            evaluation: self.definition_origin(call_span).into(),
        };
        for statement in prepared.template.body().statements() {
            sink.push(self.materialize_imported_default_statement(statement, &mut context)?);
        }
        debug_assert!(context.loop_targets.is_empty());
        self.materialize_imported_default_expression(prepared.template.body().value(), &mut context)
    }

    fn materialize_imported_default_expression(
        &mut self,
        expression: &hir::DefaultExpressionV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::Expr, ImportedDefaultMaterializationError> {
        let definition =
            self.imported_default_definition_origin(expression.definition_origin(), context)?;
        let span = definition.span;
        let origin = hir::ExpressionOrigin::Instantiated(hir::ConcreteExpressionOrigin {
            definition,
            evaluation: context.evaluation,
        });
        let ty = self
            .imported_default_type(expression.result_type())
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;

        use hir::DefaultExpressionKindV1 as Kind;
        let kind = match expression.kind() {
            Kind::SingletonValue(value) => hir::ExprKind::ImportedSingletonValue(*value),
            Kind::ReferenceUpcast(operand) => hir::ExprKind::ReferenceUpcast(Box::new(
                self.materialize_imported_default_expression(operand, context)?,
            )),
            Kind::Box(operand) => {
                let operand = self.materialize_imported_default_expression(operand, context)?;
                self.retain_imported_box_source(operand.ty)
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(
                            error.diagnostic("boxed value type"),
                        )
                    })?;
                hir::ExprKind::Box(Box::new(operand))
            }
            Kind::Unbox(operand) => hir::ExprKind::Unbox(Box::new(
                self.materialize_imported_default_expression(operand, context)?,
            )),
            Kind::IsInstance {
                operand,
                checked_type,
            } => hir::ExprKind::IsInstance {
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
                check_ty: self.imported_default_type(checked_type).map_err(|error| {
                    ImportedDefaultMaterializationError::Plan(error.to_string())
                })?,
            },
            Kind::Cast {
                operand,
                checked_type,
                optional,
            } => {
                let optional = bool::from(*optional);
                if !optional {
                    self.prepare_cast_exception_type().map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(format!(
                            "cannot resolve cast exception type: {error:?}"
                        ))
                    })?;
                }
                hir::ExprKind::Cast {
                    operand: Box::new(
                        self.materialize_imported_default_expression(operand, context)?,
                    ),
                    check_ty: self.imported_default_type(checked_type).map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?,
                    optional,
                }
            }
            Kind::StringLiteral {
                value,
                owner: hir::DefaultStringOwnerV1::CurrentInstantiation,
            } => hir::ExprKind::StringLiteral {
                value: value.clone(),
                owner: hir::StringConstantOwner::CurrentDefinition,
            },
            Kind::IntegerLiteral(value) => hir::ExprKind::IntegerLiteral((*value).into()),
            Kind::BooleanLiteral(value) => hir::ExprKind::BoolLiteral((*value).into()),
            Kind::UnitLiteral => hir::ExprKind::UnitLiteral,
            Kind::TupleLiteral(elements) => hir::ExprKind::TupleLiteral(
                self.materialize_imported_default_expressions(elements, context)?,
            ),
            Kind::VariantConstruct { variant, arguments } => {
                hir::ExprKind::ImportedVariantConstruct {
                    owner: self
                        .imported_default_type(variant.owner_type())
                        .map_err(|error| {
                            ImportedDefaultMaterializationError::Plan(error.to_string())
                        })?,
                    variant: variant.declaration(),
                    args: self.materialize_imported_default_expressions(arguments, context)?,
                }
            }
            Kind::Local(local) => {
                let mut value = context.locals.get(local).cloned().ok_or_else(|| {
                    ImportedDefaultMaterializationError::UnknownLocal(local.clone())
                })?;
                value.ty = ty;
                value.span = span;
                value.origin = origin;
                return Ok(value);
            }
            Kind::Call {
                callee,
                arguments,
                receiver,
            } => {
                let args = self.materialize_imported_default_expressions(arguments, context)?;
                let receiver = receiver
                    .as_ref()
                    .try_map(|ty| self.imported_default_type(ty))
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?;
                self.imported_default_call_kind(
                    super::plan::default_callable_origin(callee).map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?,
                    args,
                    receiver,
                    MemberCallKind::Ordinary,
                    context,
                )?
            }
            Kind::StructInit {
                constructor: hir::DefaultConstructorRefV1::Struct { declaration, .. },
                arguments,
            }
            | Kind::ClassInit {
                constructor:
                    hir::DefaultConstructorRefV1::Class {
                        declaration: hir::DefaultClassConstructorIdV1::Source(declaration),
                        ..
                    },
                arguments,
            } => {
                let args = self.materialize_imported_default_expressions(arguments, context)?;
                self.imported_default_call_kind(
                    scoop_identity::CallableTemplateOrigin::Constructor(*declaration),
                    args,
                    hir::SourceCallReceiver::NoReceiver,
                    MemberCallKind::Ordinary,
                    context,
                )?
            }
            Kind::FieldAccess {
                receiver,
                field: hir::DefaultFieldRefV1::Tuple { declaration_index },
            } => hir::ExprKind::FieldAccess {
                receiver: Box::new(
                    self.materialize_imported_default_expression(receiver, context)?,
                ),
                field: hir::FieldRef::TupleIndex(*declaration_index),
            },
            Kind::FieldAccess {
                receiver,
                field:
                    hir::DefaultFieldRefV1::Struct {
                        declaration,
                        owner_type,
                    },
            } => {
                let owner = self.imported_default_type(owner_type).map_err(|error| {
                    ImportedDefaultMaterializationError::Plan(error.to_string())
                })?;
                hir::ExprKind::FieldAccess {
                    receiver: Box::new(
                        self.materialize_imported_default_expression(receiver, context)?,
                    ),
                    field: hir::FieldRef::ImportedStruct {
                        owner,
                        field: *declaration,
                    },
                }
            }
            Kind::MethodCall {
                receiver,
                callee: hir::DefaultMethodCalleeV1::Callable(callee),
                arguments,
            }
            | Kind::DirectSuperMethodCall {
                receiver,
                callee: hir::DefaultMethodCalleeV1::Callable(callee),
                arguments,
            } => {
                let kind = if matches!(expression.kind(), Kind::DirectSuperMethodCall { .. }) {
                    MemberCallKind::DirectSuper
                } else {
                    MemberCallKind::Ordinary
                };
                let receiver = self.materialize_imported_default_expression(receiver, context)?;
                let receiver_type = receiver.ty;
                let mut args = vec![receiver];
                args.extend(self.materialize_imported_default_expressions(arguments, context)?);
                self.imported_default_call_kind(
                    super::plan::default_callable_origin(callee).map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(error.to_string())
                    })?,
                    args,
                    hir::SourceCallReceiver::Receiver {
                        static_type: receiver_type,
                    },
                    kind,
                    context,
                )?
            }
            Kind::PrimitiveBinary { kind, lhs, rhs } => hir::ExprKind::PrimitiveBinary {
                kind: (*kind).into(),
                lhs: Box::new(self.materialize_imported_default_expression(lhs, context)?),
                rhs: Box::new(self.materialize_imported_default_expression(rhs, context)?),
            },
            Kind::PrimitiveUnary { kind, operand } => hir::ExprKind::PrimitiveUnary {
                kind: (*kind).into(),
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
            },
            Kind::IntegerOperation {
                operation,
                arguments,
            } => {
                if matches!(operation, hir::DefaultIntegerOperationV1::Managed { .. }) {
                    self.prepare_arithmetic_exception_type().map_err(|error| {
                        ImportedDefaultMaterializationError::Plan(
                            error.diagnostic("integer division exception type"),
                        )
                    })?;
                }
                hir::ExprKind::IntegerOperation {
                    operation: (*operation).into(),
                    arguments: match arguments {
                        hir::DefaultIntegerArgumentsV1::Unary(operand) => {
                            hir::HirIntegerOperationArguments::Unary(Box::new(
                                self.materialize_imported_default_expression(operand, context)?,
                            ))
                        }
                        hir::DefaultIntegerArgumentsV1::Binary { lhs, rhs } => {
                            hir::HirIntegerOperationArguments::Binary {
                                lhs: Box::new(
                                    self.materialize_imported_default_expression(lhs, context)?,
                                ),
                                rhs: Box::new(
                                    self.materialize_imported_default_expression(rhs, context)?,
                                ),
                            }
                        }
                    },
                }
            }
            Kind::IntegerConversion {
                source_kind,
                target_kind,
                operand,
            } => hir::ExprKind::IntegerConversion {
                conversion: hir::IntegerConversion {
                    source: (*source_kind).into(),
                    target_kind: (*target_kind).into(),
                },
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
            },
            Kind::Binary { operator, lhs, rhs } => hir::ExprKind::Binary {
                op: (*operator).into(),
                lhs: Box::new(self.materialize_imported_default_expression(lhs, context)?),
                rhs: Box::new(self.materialize_imported_default_expression(rhs, context)?),
            },
            Kind::Unary { operator, operand } => hir::ExprKind::Unary {
                op: (*operator).into(),
                operand: Box::new(self.materialize_imported_default_expression(operand, context)?),
            },
            Kind::StringLiteral {
                owner: hir::DefaultStringOwnerV1::Property(_),
                ..
            }
            | Kind::StructInit { .. }
            | Kind::StructConstruct { .. }
            | Kind::ClassInit { .. }
            | Kind::VariantTest { .. }
            | Kind::VariantPayloadProject { .. }
            | Kind::GlobalRead(_)
            | Kind::Lambda(_)
            | Kind::AnonymousFunction(_)
            | Kind::CallableReference(_)
            | Kind::FunctionCoercion { .. }
            | Kind::PtrFromNonZeroULong(_)
            | Kind::PtrToULong(_)
            | Kind::PtrCast(_)
            | Kind::PtrLoad { .. }
            | Kind::PtrStore { .. }
            | Kind::PtrOffset { .. }
            | Kind::AddressOf(_)
            | Kind::SizeOf(_)
            | Kind::AlignOf(_)
            | Kind::FunctionAddress(_)
            | Kind::ForeignCallbackRegister { .. }
            | Kind::ForeignCallbackOperation { .. }
            | Kind::FieldAccess { .. }
            | Kind::MethodCall { .. }
            | Kind::DirectSuperMethodCall { .. }
            | Kind::ArrayLiteral(_)
            | Kind::ArrayAssembly(_)
            | Kind::Index { .. }
            | Kind::ArraySet { .. }
            | Kind::ArrayLen(_)
            | Kind::ArrayClone(_)
            | Kind::LocalFunctionCall { .. }
            | Kind::CallableCall { .. }
            | Kind::SomeWrap(_)
            | Kind::NoneLiteral
            | Kind::IsSome(_)
            | Kind::Unwrap { .. } => {
                return Err(ImportedDefaultMaterializationError::Plan(
                    "preflight admitted an unsupported dependency default operation".to_owned(),
                ));
            }
        };
        Ok(hir::Expr {
            kind,
            ty,
            span,
            origin,
        })
    }

    fn materialize_imported_default_expressions(
        &mut self,
        expressions: &[hir::DefaultExpressionV1],
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<Vec<hir::Expr>, ImportedDefaultMaterializationError> {
        expressions
            .iter()
            .map(|expression| self.materialize_imported_default_expression(expression, context))
            .collect()
    }

    fn imported_default_call_kind(
        &mut self,
        callee: scoop_identity::CallableTemplateOrigin,
        args: Vec<hir::Expr>,
        receiver: hir::SourceCallReceiver<hir::TypeId>,
        kind: MemberCallKind,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        let candidate = context
            .prepared
            .callables
            .get(&callee)
            .ok_or(ImportedDefaultMaterializationError::MissingCallable(callee))?;
        let callee = self
            .select_imported_callable_declaration_use_with_kind(candidate.clone(), kind)
            .map_err(|error| {
                ImportedDefaultMaterializationError::DependencySelection(error.to_string())
            })?;
        Ok(hir::ExprKind::ImportedDependencyCall {
            callee,
            binding: None,
            args,
            receiver,
        })
    }

    fn imported_default_definition_origin(
        &mut self,
        source: &hir::ExportDefinitionSourceV1,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::DefinitionOrigin, ImportedDefaultMaterializationError> {
        let imported = context.owner.definition_source(source).ok_or_else(|| {
            ImportedDefinitionOriginError::MissingSource {
                context: source.origin().context(),
            }
        })?;
        self.import_dependency_definition_origin(source, imported)
            .map_err(Into::into)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ImportedDefaultMaterializationError {
    MissingReceiver,
    ParameterIndexOverflow,
    MissingValueParameter { position: u32 },
    UnknownLocal(LocalValueSelector),
    ExpectedMaterializedLocal(LocalValueSelector),
    InvalidControlFlow(&'static str),
    MissingCallable(scoop_identity::CallableTemplateOrigin),
    DependencySelection(String),
    DefinitionOrigin(ImportedDefinitionOriginError),
    Plan(String),
}

impl From<ImportedDefinitionOriginError> for ImportedDefaultMaterializationError {
    fn from(value: ImportedDefinitionOriginError) -> Self {
        Self::DefinitionOrigin(value)
    }
}

impl fmt::Display for ImportedDefaultMaterializationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingReceiver => {
                formatter.write_str("dependency default is missing its receiver value")
            }
            Self::ParameterIndexOverflow => {
                formatter.write_str("dependency default parameter index exceeds usize")
            }
            Self::MissingValueParameter { position } => write!(
                formatter,
                "dependency default is missing prior value parameter {position}"
            ),
            Self::UnknownLocal(local) => {
                write!(
                    formatter,
                    "dependency default reads unmapped local {local:?}"
                )
            }
            Self::ExpectedMaterializedLocal(local) => write!(
                formatter,
                "dependency default local {local:?} is not backed by a materialized local"
            ),
            Self::InvalidControlFlow(operation) => {
                write!(
                    formatter,
                    "invalid dependency default control flow: {operation}"
                )
            }
            Self::MissingCallable(callee) => {
                write!(
                    formatter,
                    "dependency default call {callee:?} was not preflighted"
                )
            }
            Self::DependencySelection(error) => {
                write!(
                    formatter,
                    "dependency default callable selection failed: {error}"
                )
            }
            Self::DefinitionOrigin(error) => error.fmt(formatter),
            Self::Plan(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for ImportedDefaultMaterializationError {}

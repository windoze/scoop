use std::collections::BTreeMap;
use std::fmt;

use scoop_ast::Span;
use scoop_hir as hir;
use scoop_identity::LocalValueSelector;

use super::plan::{PreparedImportedDefault, PreparedImportedDefaultCallable};
use crate::Lowerer;
use crate::expr::imported_origins::ImportedDefinitionOriginError;

struct ImportedDefaultContext<'a> {
    owner: &'a hir::ImportedDependencyCallableCandidate,
    prepared: &'a PreparedImportedDefault,
    locals: BTreeMap<LocalValueSelector, hir::Expr>,
    evaluation: hir::EvaluationOrigin,
}

impl Lowerer {
    pub(in super::super) fn materialize_imported_default(
        &mut self,
        owner: &hir::ImportedDependencyCallableCandidate,
        prepared: &PreparedImportedDefault,
        receiver: Option<&hir::Expr>,
        value_parameters: &[hir::Expr],
        call_span: Span,
    ) -> Result<hir::Expr, ImportedDefaultMaterializationError> {
        if !prepared.template.body().statements().is_empty() {
            return Err(ImportedDefaultMaterializationError::UnexpectedStatements);
        }
        let mut locals = BTreeMap::new();
        if let Some(template_receiver) = prepared.template.receiver().receiver() {
            let value = receiver
                .cloned()
                .ok_or(ImportedDefaultMaterializationError::MissingReceiver)?;
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
        let mut context = ImportedDefaultContext {
            owner,
            prepared,
            locals,
            evaluation: self.definition_origin(call_span).into(),
        };
        self.materialize_imported_default_expression(prepared.template.body().value(), &mut context)
    }

    fn materialize_imported_default_expression(
        &mut self,
        expression: &hir::DefaultExpressionV1,
        context: &mut ImportedDefaultContext<'_>,
    ) -> Result<hir::Expr, ImportedDefaultMaterializationError> {
        let source = expression.definition_origin();
        let imported = context.owner.definition_source(source).ok_or_else(|| {
            ImportedDefinitionOriginError::MissingAuthenticatedSource {
                context: source.origin().context(),
            }
        })?;
        let definition = self.import_dependency_definition_origin(source, imported)?;
        let span = definition.span;
        let origin = hir::ExpressionOrigin::Instantiated(hir::ConcreteExpressionOrigin {
            definition,
            evaluation: context.evaluation,
        });
        let ty = self
            .imported_default_core_type(expression.result_type())
            .map_err(|error| ImportedDefaultMaterializationError::Plan(error.to_string()))?;

        use hir::DefaultExpressionKindV1 as Kind;
        let kind = match expression.kind() {
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
            Kind::Local(local) => {
                let mut value = context.locals.get(local).cloned().ok_or_else(|| {
                    ImportedDefaultMaterializationError::UnknownLocal(local.clone())
                })?;
                value.ty = ty;
                value.span = span;
                value.origin = origin;
                return Ok(value);
            }
            Kind::Call { callee, arguments } => {
                let args = self.materialize_imported_default_expressions(arguments, context)?;
                self.imported_default_call_kind(callee, args, span, context)?
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
                let callee = match operation {
                    hir::DefaultIntegerOperationV1::NoGc { target, .. }
                    | hir::DefaultIntegerOperationV1::Managed { target, .. } => target,
                };
                let args = match arguments {
                    hir::DefaultIntegerArgumentsV1::Unary(operand) => {
                        vec![self.materialize_imported_default_expression(operand, context)?]
                    }
                    hir::DefaultIntegerArgumentsV1::Binary { lhs, rhs } => vec![
                        self.materialize_imported_default_expression(lhs, context)?,
                        self.materialize_imported_default_expression(rhs, context)?,
                    ],
                };
                self.imported_default_call_kind(callee, args, span, context)?
            }
            Kind::IntegerConversion {
                target, operand, ..
            } => {
                let args = vec![self.materialize_imported_default_expression(operand, context)?];
                self.imported_default_call_kind(target, args, span, context)?
            }
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
            | Kind::TupleLiteral(_)
            | Kind::StructInit { .. }
            | Kind::StructConstruct { .. }
            | Kind::ClassInit { .. }
            | Kind::VariantConstruct { .. }
            | Kind::VariantTest { .. }
            | Kind::VariantPayloadProject { .. }
            | Kind::GlobalRead(_)
            | Kind::SingletonValue(_)
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
        callee: &hir::DefaultCallableRefV1,
        args: Vec<hir::Expr>,
        span: Span,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::ExprKind, ImportedDefaultMaterializationError> {
        let target =
            context.prepared.callables.get(callee).ok_or_else(|| {
                ImportedDefaultMaterializationError::MissingCallable(callee.clone())
            })?;
        match target {
            PreparedImportedDefaultCallable::Core(reference) => {
                let callee = self
                    .select_imported_core_callable(*reference, span)
                    .ok_or(ImportedDefaultMaterializationError::CoreSelection)?;
                Ok(hir::ExprKind::ImportedCoreCall { callee, args })
            }
            PreparedImportedDefaultCallable::Dependency(candidate) => {
                let reference = self
                    .dependencies
                    .as_mut()
                    .expect("ordinary lowering carries a dependency selection plan")
                    .select_callable(candidate.as_ref().clone())
                    .map_err(|error| {
                        ImportedDefaultMaterializationError::DependencySelection(error.to_string())
                    })?;
                let existing = self
                    .imported_dependency_callables
                    .iter()
                    .find_map(|(id, use_)| (use_.reference() == reference).then_some(id));
                let callee = existing.unwrap_or_else(|| {
                    self.imported_dependency_callables
                        .alloc(hir::ImportedDependencyCallableUse::new(reference))
                });
                Ok(hir::ExprKind::ImportedDependencyCall { callee, args })
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum ImportedDefaultMaterializationError {
    UnexpectedStatements,
    MissingReceiver,
    ParameterIndexOverflow,
    MissingValueParameter { position: u32 },
    UnknownLocal(LocalValueSelector),
    MissingCallable(hir::DefaultCallableRefV1),
    CoreSelection,
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
            Self::UnexpectedStatements => {
                formatter.write_str("preflight admitted dependency default statements")
            }
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
            Self::MissingCallable(callee) => {
                write!(
                    formatter,
                    "dependency default call {callee:?} was not preflighted"
                )
            }
            Self::CoreSelection => {
                formatter.write_str("dependency default core callable selection failed")
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

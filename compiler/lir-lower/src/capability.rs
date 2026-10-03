//! Reachable persistent-materialization checks for the strong LIR profile.

use std::fmt;

use scoop_mir as mir;

use crate::{dependency_types::DependencyTypeDescriptors, identity_roots::IdentityRoots};

/// One persistent entity required by reachable MIR but unavailable without
/// independent generic or structural ownership.
#[derive(Clone, Debug, PartialEq)]
pub enum StrongLirMaterializationRequirement {
    TypeDescriptor(mir::Type),
    ArrayType(mir::ClassId),
}

/// A reachable edge that the SingleConeStrong profile cannot materialize.
#[derive(Clone, Debug, PartialEq)]
pub struct StrongLirCapabilityError {
    function: mir::FunctionId,
    requirement: StrongLirMaterializationRequirement,
}

impl StrongLirCapabilityError {
    pub const CODE: &'static str = "SCOOPC_CAPABILITY_ODR_UNAVAILABLE";

    pub const fn function(&self) -> mir::FunctionId {
        self.function
    }

    pub const fn requirement(&self) -> &StrongLirMaterializationRequirement {
        &self.requirement
    }
}

impl fmt::Display for StrongLirCapabilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: reachable MIR function {:?} requires independent {:?} materialization, which the SingleConeStrong profile forbids",
            Self::CODE,
            self.function,
            self.requirement
        )
    }
}

impl std::error::Error for StrongLirCapabilityError {}

pub(crate) fn validate_strong_materialization(
    input: &mir::ConeMirInput,
    roots: &IdentityRoots<'_>,
    dependencies: &DependencyTypeDescriptors,
) -> Result<(), StrongLirCapabilityError> {
    let module = input.module();
    for root in input.materialization().callable_roots() {
        let function_id = root.function();
        let function = &module.functions[function_id];
        for (_, block) in function.body.blocks.iter() {
            for statement in &block.statements {
                validate_statement(module, roots, dependencies, function_id, statement)?;
            }
            validate_terminator(module, roots, dependencies, function_id, &block.terminator)?;
        }
    }
    Ok(())
}

fn validate_statement(
    module: &mir::Module,
    roots: &IdentityRoots<'_>,
    dependencies: &DependencyTypeDescriptors,
    function: mir::FunctionId,
    statement: &mir::Statement,
) -> Result<(), StrongLirCapabilityError> {
    match &statement.kind {
        mir::StatementKind::Expr(expr) => {
            validate_expr(module, roots, dependencies, function, expr)
        }
        mir::StatementKind::PublishReleaseReady { receiver, .. } => {
            validate_expr(module, roots, dependencies, function, receiver)
        }
        mir::StatementKind::Call(effect) => {
            let call = match effect {
                mir::CallEffect::Unit(call) | mir::CallEffect::Value { call, .. } => call,
            };
            validate_call(module, roots, dependencies, function, call)
        }
        mir::StatementKind::ValDecl { init, .. } => {
            validate_expr(module, roots, dependencies, function, init)
        }
        mir::StatementKind::Assign { value, .. }
        | mir::StatementKind::GlobalAssign { value, .. } => {
            validate_expr(module, roots, dependencies, function, value)
        }
        mir::StatementKind::ArraySet {
            array_type,
            array,
            index,
            value,
        } => {
            require_array(roots, function, *array_type)?;
            validate_expr(module, roots, dependencies, function, array)?;
            validate_expr(module, roots, dependencies, function, index)?;
            validate_expr(module, roots, dependencies, function, value)
        }
        mir::StatementKind::FieldSet { object, value, .. }
        | mir::StatementKind::AtomicFieldStore { object, value, .. } => {
            validate_expr(module, roots, dependencies, function, object)?;
            validate_expr(module, roots, dependencies, function, value)
        }
        mir::StatementKind::Eh(_) => Ok(()),
    }
}

fn validate_call(
    module: &mir::Module,
    roots: &IdentityRoots<'_>,
    dependencies: &DependencyTypeDescriptors,
    function: mir::FunctionId,
    call: &mir::Call,
) -> Result<(), StrongLirCapabilityError> {
    match call.target.kind {
        mir::CallKind::Interface { interface, .. } => {
            require_descriptor(
                module,
                roots,
                dependencies,
                function,
                &mir::Type::Interface(interface),
            )?;
        }
        mir::CallKind::FunctionBridge { .. }
        | mir::CallKind::Direct
        | mir::CallKind::Virtual { .. }
        | mir::CallKind::Closure { .. } => {}
    }
    for argument in &call.args {
        validate_expr(module, roots, dependencies, function, argument)?;
    }
    Ok(())
}

fn validate_terminator(
    module: &mir::Module,
    roots: &IdentityRoots<'_>,
    dependencies: &DependencyTypeDescriptors,
    function: mir::FunctionId,
    terminator: &mir::Terminator,
) -> Result<(), StrongLirCapabilityError> {
    match terminator {
        mir::Terminator::Branch { cond, .. } => {
            validate_expr(module, roots, dependencies, function, cond)
        }
        mir::Terminator::Return { value: Some(value) } => {
            validate_expr(module, roots, dependencies, function, value)
        }
        mir::Terminator::Throw { exception, .. } => {
            validate_expr(module, roots, dependencies, function, exception)
        }
        mir::Terminator::Goto(_)
        | mir::Terminator::Return { value: None }
        | mir::Terminator::Rethrow { .. }
        | mir::Terminator::Resume
        | mir::Terminator::Trap { .. }
        | mir::Terminator::Unreachable => Ok(()),
    }
}

fn validate_expr(
    module: &mir::Module,
    roots: &IdentityRoots<'_>,
    dependencies: &DependencyTypeDescriptors,
    function: mir::FunctionId,
    expression: &mir::Expr,
) -> Result<(), StrongLirCapabilityError> {
    let mut failure = None;
    mir::visit_expr(expression, &mut |expression| {
        if failure.is_none() {
            failure = expression_requirement(module, roots, dependencies, expression).map(
                |requirement| StrongLirCapabilityError {
                    function,
                    requirement,
                },
            );
        }
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn expression_requirement(
    module: &mir::Module,
    roots: &IdentityRoots<'_>,
    dependencies: &DependencyTypeDescriptors,
    expression: &mir::Expr,
) -> Option<StrongLirMaterializationRequirement> {
    match &expression.kind {
        mir::ExprKind::ClassAlloc { class_id } => {
            unavailable_descriptor(module, roots, dependencies, &mir::Type::Class(*class_id))
        }
        mir::ExprKind::Box(operand) => {
            unavailable_descriptor(module, roots, dependencies, &operand.ty)
        }
        mir::ExprKind::Unbox(_) => {
            unavailable_descriptor(module, roots, dependencies, &expression.ty)
        }
        mir::ExprKind::IsInstance { check_ty, .. } => {
            unavailable_descriptor(module, roots, dependencies, check_ty)
        }
        mir::ExprKind::ArrayLiteral { array_type, .. }
        | mir::ExprKind::ArrayAssembly { array_type, .. }
        | mir::ExprKind::ArrayGet { array_type, .. }
        | mir::ExprKind::ArrayLen { array_type, .. } => unavailable_array(roots, *array_type),
        mir::ExprKind::ArrayClone {
            source_type,
            target_type,
            ..
        } => unavailable_array(roots, *source_type)
            .or_else(|| unavailable_array(roots, *target_type)),
        mir::ExprKind::StringConst(_)
        | mir::ExprKind::IntegerLiteral(_)
        | mir::ExprKind::MachineScalarLiteral(_)
        | mir::ExprKind::BoolLiteral(_)
        | mir::ExprKind::UnitLiteral
        | mir::ExprKind::ReleaseFieldLoad { .. }
        | mir::ExprKind::TupleLiteral(_)
        | mir::ExprKind::StructInit { .. }
        | mir::ExprKind::StructConstruct { .. }
        | mir::ExprKind::ClosureAlloc { .. }
        | mir::ExprKind::ClosureCapture { .. }
        | mir::ExprKind::Local(_)
        | mir::ExprKind::GlobalRead(_)
        | mir::ExprKind::InitializationUnitAddress(_)
        | mir::ExprKind::PtrFromNonZeroULong { .. }
        | mir::ExprKind::PtrToULong(_)
        | mir::ExprKind::PtrCast { .. }
        | mir::ExprKind::PtrLoad { .. }
        | mir::ExprKind::PtrStore { .. }
        | mir::ExprKind::PtrOffset { .. }
        | mir::ExprKind::AddressOf { .. }
        | mir::ExprKind::GlobalAddress { .. }
        | mir::ExprKind::SizeOf(_)
        | mir::ExprKind::AlignOf(_)
        | mir::ExprKind::FunctionAddress { .. }
        | mir::ExprKind::ForeignCallbackRegister { .. }
        | mir::ExprKind::ForeignCallbackOperation { .. }
        | mir::ExprKind::CaughtException
        | mir::ExprKind::Retype { .. }
        | mir::ExprKind::FieldAccess { .. }
        | mir::ExprKind::AtomicFieldLoad { .. }
        | mir::ExprKind::AtomicFieldCompareExchange { .. }
        | mir::ExprKind::Cast { .. }
        | mir::ExprKind::Binary { .. }
        | mir::ExprKind::Unary { .. }
        | mir::ExprKind::IntegerUnary { .. }
        | mir::ExprKind::IntegerBinary { .. }
        | mir::ExprKind::SafeIntegerDivRem { .. }
        | mir::ExprKind::IntegerCompare { .. }
        | mir::ExprKind::IntegerCompareTo { .. }
        | mir::ExprKind::IntegerShift { .. }
        | mir::ExprKind::IntegerConversion { .. }
        | mir::ExprKind::VariantConstruct { .. }
        | mir::ExprKind::EnumTag(_)
        | mir::ExprKind::EnumField { .. }
        | mir::ExprKind::VariantTest { .. }
        | mir::ExprKind::VariantPayloadProject { .. } => None,
    }
}

fn require_descriptor(
    module: &mir::Module,
    roots: &IdentityRoots<'_>,
    dependencies: &DependencyTypeDescriptors,
    function: mir::FunctionId,
    ty: &mir::Type,
) -> Result<(), StrongLirCapabilityError> {
    match unavailable_descriptor(module, roots, dependencies, ty) {
        Some(requirement) => Err(StrongLirCapabilityError {
            function,
            requirement,
        }),
        None => Ok(()),
    }
}

fn unavailable_descriptor(
    module: &mir::Module,
    roots: &IdentityRoots<'_>,
    dependencies: &DependencyTypeDescriptors,
    ty: &mir::Type,
) -> Option<StrongLirMaterializationRequirement> {
    let available = match ty {
        mir::Type::Class(_) | mir::Type::Interface(_) | mir::Type::String => {
            roots.materializes_type(ty) || dependencies.contains(ty)
        }
        mir::Type::Struct(_)
        | mir::Type::Enum(..)
        | mir::Type::Tuple(_)
        | mir::Type::Integer(_)
        | mir::Type::MachineScalar(_)
        | mir::Type::Boolean
        | mir::Type::Unit
        | mir::Type::Ptr(_)
        | mir::Type::FunPtr(_) => module.meta.boxed_types.iter().any(|boxed| {
            boxed.payload() == ty
                && (roots.materializes_type(&mir::Type::Class(boxed.class()))
                    || dependencies.contains(&mir::Type::Class(boxed.class())))
        }),
        mir::Type::Function(_) => module.meta.source_exact_types.get(ty).is_some(),
        mir::Type::Any => false,
    };
    (!available).then(|| StrongLirMaterializationRequirement::TypeDescriptor(ty.clone()))
}

fn require_array(
    roots: &IdentityRoots<'_>,
    function: mir::FunctionId,
    class: mir::ClassId,
) -> Result<(), StrongLirCapabilityError> {
    match unavailable_array(roots, class) {
        Some(requirement) => Err(StrongLirCapabilityError {
            function,
            requirement,
        }),
        None => Ok(()),
    }
}

fn unavailable_array(
    roots: &IdentityRoots<'_>,
    class: mir::ClassId,
) -> Option<StrongLirMaterializationRequirement> {
    (!roots.materializes_type(&mir::Type::Class(class)))
        .then_some(StrongLirMaterializationRequirement::ArrayType(class))
}

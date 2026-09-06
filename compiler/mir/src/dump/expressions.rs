use super::super::*;
use super::type_name;

pub(super) fn dump_expr(
    module: &Module,
    locals: &Arena<Local>,
    expr: &Expr,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    out.push_str(&format!("{pad}Type {}\n", type_name(module, &expr.ty)));
    match &expr.kind {
        ExprKind::StringConst(id) => {
            out.push_str(&format!(
                "{pad}StringConst @{}\n",
                module.strings[*id].symbol
            ));
        }
        ExprKind::IntegerLiteral(value) => {
            let digits = usize::from(value.width().bytes()) * 2;
            out.push_str(&format!(
                "{pad}IntegerLiteral {} value={} bits=0x{:0digits$x}\n",
                value.kind().canonical_name(),
                value.mathematical_value(),
                value.raw_bits(),
            ));
        }
        ExprKind::MachineScalarLiteral(value) => {
            out.push_str(&format!("{pad}MachineScalarLiteral {value:?}\n"));
        }
        ExprKind::BoolLiteral(value) => out.push_str(&format!("{pad}BoolLiteral {value}\n")),
        ExprKind::UnitLiteral => out.push_str(&format!("{pad}UnitLiteral\n")),
        ExprKind::InitializationUnitAddress(unit) => out.push_str(&format!(
            "{pad}InitializationUnitAddress init{} {}\n",
            unit.into_raw().into_u32(),
            module.initialization_units[*unit].stable_key
        )),
        ExprKind::TupleLiteral(elements) => {
            out.push_str(&format!("{pad}TupleLiteral\n"));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::ClassAlloc { class_id } => out.push_str(&format!(
            "{pad}ClassAlloc {}\n",
            module.classes[*class_id].name
        )),
        ExprKind::ClosureAlloc { class, captures } => {
            out.push_str(&format!(
                "{pad}ClosureAlloc cc{} {}\n",
                class.into_raw().into_u32(),
                module.closure_classes[*class].name
            ));
            for capture in captures {
                dump_expr(module, locals, capture, indent + 1, out);
            }
        }
        ExprKind::ClosureCapture {
            closure,
            class,
            index,
        } => {
            out.push_str(&format!(
                "{pad}ClosureCapture cc{} {index}\n",
                class.into_raw().into_u32()
            ));
            dump_expr(module, locals, closure, indent + 1, out);
        }
        ExprKind::StructInit { struct_id, args } => {
            out.push_str(&format!(
                "{pad}StructInit {}\n",
                module.structs[*struct_id].name
            ));
            for arg in args {
                dump_expr(module, locals, arg, indent + 1, out);
            }
        }
        ExprKind::Local(local) => out.push_str(&format!("{pad}Local {}\n", locals[*local].name)),
        ExprKind::GlobalRead(global) => out.push_str(&format!(
            "{pad}GlobalRead {}\n",
            module.globals[*global].name
        )),
        ExprKind::PtrFromNonZeroULong { operand, pointee } => {
            out.push_str(&format!(
                "{pad}PtrFromNonZeroULong {}\n",
                type_name(module, pointee)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrToULong(operand) => {
            out.push_str(&format!("{pad}PtrToULong\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrCast { operand, pointee } => {
            out.push_str(&format!("{pad}PtrCast {}\n", type_name(module, pointee)));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::PtrLoad {
            pointer,
            pointee,
            offset,
        } => {
            out.push_str(&format!("{pad}PtrLoad {}\n", type_name(module, pointee)));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
        }
        ExprKind::PtrStore {
            pointer,
            pointee,
            offset,
            value,
        } => {
            out.push_str(&format!("{pad}PtrStore {}\n", type_name(module, pointee)));
            dump_expr(module, locals, pointer, indent + 1, out);
            if let Some(offset) = offset {
                dump_expr(module, locals, offset, indent + 1, out);
            }
            dump_expr(module, locals, value, indent + 1, out);
        }
        ExprKind::PtrOffset {
            pointer,
            pointee,
            offset,
            subtract,
        } => {
            out.push_str(&format!(
                "{pad}PtrOffset {} subtract={subtract}\n",
                type_name(module, pointee)
            ));
            dump_expr(module, locals, pointer, indent + 1, out);
            dump_expr(module, locals, offset, indent + 1, out);
        }
        ExprKind::AddressOf { local, pointee } => out.push_str(&format!(
            "{pad}AddressOf {} : Ptr<{}>\n",
            locals[*local].name,
            type_name(module, pointee)
        )),
        ExprKind::GlobalAddress { global, pointee } => out.push_str(&format!(
            "{pad}GlobalAddress {} {}\n",
            module.globals[*global].name,
            type_name(module, pointee)
        )),
        ExprKind::SizeOf(ty) => {
            out.push_str(&format!("{pad}SizeOf {}\n", type_name(module, ty)));
        }
        ExprKind::AlignOf(ty) => {
            out.push_str(&format!("{pad}AlignOf {}\n", type_name(module, ty)));
        }
        ExprKind::FunctionAddress { callback } => out.push_str(&format!(
            "{pad}FunctionAddress cb{}\n",
            callback.into_raw().into_u32()
        )),
        ExprKind::ForeignCallbackRegister { bridge, closure } => {
            let bridge_id = *bridge;
            let bridge = &module.foreign_callback_bridges[bridge_id];
            out.push_str(&format!(
                "{pad}ForeignCallbackRegister fcb{} family=fcf{}\n",
                bridge_id.into_raw().into_u32(),
                bridge.family.into_raw().into_u32(),
            ));
            dump_expr(module, locals, closure, indent + 1, out);
        }
        ExprKind::ForeignCallbackOperation {
            operation,
            callback,
            ..
        } => {
            out.push_str(&format!(
                "{pad}ForeignCallback{} family{}\n",
                operation.name(),
                operation.family().into_raw().into_u32()
            ));
            dump_expr(module, locals, callback, indent + 1, out);
        }
        ExprKind::CaughtException => out.push_str(&format!("{pad}CaughtException\n")),
        ExprKind::Retype { operand, ty } => {
            out.push_str(&format!("{pad}Retype {}\n", type_name(module, ty)));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::FieldAccess { receiver, index } => {
            out.push_str(&format!("{pad}FieldAccess {index}\n"));
            dump_expr(module, locals, receiver, indent + 1, out);
        }
        ExprKind::AtomicFieldLoad {
            kind,
            object,
            index,
        } => {
            out.push_str(&format!(
                "{pad}AtomicLoadAcquire kind={} field={index}\n",
                kind.name()
            ));
            dump_expr(module, locals, object, indent + 1, out);
        }
        ExprKind::AtomicFieldCompareExchange {
            kind,
            object,
            index,
            expected,
            replacement,
        } => {
            out.push_str(&format!(
                "{pad}AtomicCompareExchange kind={} field={index} success=acq_rel failure=acquire\n",
                kind.name()
            ));
            dump_expr(module, locals, object, indent + 1, out);
            dump_expr(module, locals, expected, indent + 1, out);
            dump_expr(module, locals, replacement, indent + 1, out);
        }
        ExprKind::Box(operand) => {
            out.push_str(&format!("{pad}Box\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Unbox(operand) => {
            out.push_str(&format!("{pad}Unbox\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::IsInstance { operand, check_ty } => {
            out.push_str(&format!(
                "{pad}IsInstance {}\n",
                type_name(module, check_ty)
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Cast { operand, optional } => {
            out.push_str(&format!("{pad}Cast optional={optional}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayLiteral {
            array_type,
            elements,
        } => {
            out.push_str(&format!(
                "{pad}ArrayLiteral {}\n",
                module.classes[*array_type].name
            ));
            for element in elements {
                dump_expr(module, locals, element, indent + 1, out);
            }
        }
        ExprKind::ArrayAssembly { array_type, parts } => {
            out.push_str(&format!(
                "{pad}ArrayAssembly {}\n",
                module.classes[*array_type].name
            ));
            for part in parts {
                match part {
                    ArrayAssemblyPart::Element(value) => {
                        out.push_str(&format!("{pad}  Element\n"));
                        dump_expr(module, locals, value, indent + 2, out);
                    }
                    ArrayAssemblyPart::CopyArray(value) => {
                        out.push_str(&format!("{pad}  CopyArray\n"));
                        dump_expr(module, locals, value, indent + 2, out);
                    }
                }
            }
        }
        ExprKind::ArrayGet {
            array_type,
            array,
            index,
        } => {
            out.push_str(&format!(
                "{pad}ArrayGet {}\n",
                module.classes[*array_type].name
            ));
            dump_expr(module, locals, array, indent + 1, out);
            dump_expr(module, locals, index, indent + 1, out);
        }
        ExprKind::ArrayLen {
            array_type,
            operand,
        } => {
            out.push_str(&format!(
                "{pad}ArrayLen {}\n",
                module.classes[*array_type].name
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::ArrayClone {
            source_type,
            target_type,
            operand,
        } => {
            out.push_str(&format!(
                "{pad}ArrayClone {} -> {}\n",
                module.classes[*source_type].name, module.classes[*target_type].name
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::Binary { op, lhs, rhs } => {
            out.push_str(&format!("{pad}Binary {op:?}\n"));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::Unary { op, operand } => {
            out.push_str(&format!("{pad}Unary {op:?}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::IntegerUnary { operation, operand } => {
            out.push_str(&format!(
                "{pad}IntegerUnary {} kind={}\n",
                operation.operator().name(),
                operation.kind().canonical_name(),
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::IntegerBinary {
            operation,
            lhs,
            rhs,
        } => {
            out.push_str(&format!(
                "{pad}IntegerBinary {} kind={}\n",
                operation.operator().name(),
                operation.kind().canonical_name(),
            ));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::SafeIntegerDivRem {
            operation,
            lhs,
            rhs,
        } => {
            out.push_str(&format!(
                "{pad}SafeIntegerDivRem {} kind={}\n",
                operation.operator().name(),
                operation.kind().canonical_name(),
            ));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::IntegerCompare {
            operation,
            lhs,
            rhs,
        } => {
            out.push_str(&format!(
                "{pad}IntegerCompare {} operands={} result=Boolean\n",
                operation.operator().name(),
                operation.operand_kind().canonical_name(),
            ));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::IntegerCompareTo {
            operation,
            lhs,
            rhs,
        } => {
            out.push_str(&format!(
                "{pad}IntegerCompareTo operands={} result={}\n",
                operation.operand_kind().canonical_name(),
                operation.result_kind().canonical_name(),
            ));
            dump_expr(module, locals, lhs, indent + 1, out);
            dump_expr(module, locals, rhs, indent + 1, out);
        }
        ExprKind::IntegerShift {
            operation,
            value,
            count,
        } => {
            out.push_str(&format!(
                "{pad}IntegerShift {} value={} count={}\n",
                operation.operator().name(),
                operation.value_kind().canonical_name(),
                operation.count_kind().canonical_name(),
            ));
            dump_expr(module, locals, value, indent + 1, out);
            dump_expr(module, locals, count, indent + 1, out);
        }
        ExprKind::IntegerConversion {
            conversion,
            operand,
        } => {
            out.push_str(&format!(
                "{pad}IntegerConversion {} -> {}\n",
                conversion.source_kind().canonical_name(),
                conversion.target_kind().canonical_name(),
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::VariantConstruct { variant, fields } => {
            out.push_str(&format!(
                "{pad}VariantConstruct {} v{}\n",
                type_name(module, &expr.ty),
                variant
            ));
            for field in fields {
                dump_expr(module, locals, field, indent + 1, out);
            }
        }
        ExprKind::EnumTag(operand) => {
            out.push_str(&format!("{pad}EnumTag\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::EnumField {
            operand,
            variant,
            index,
        } => {
            out.push_str(&format!("{pad}EnumField v{variant} f{index}\n"));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::VariantTest { operand, variant } => {
            out.push_str(&format!(
                "{pad}VariantTest {} v{}\n",
                module.enums[variant.enum_id()].name,
                variant.variant_index()
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
        ExprKind::VariantPayloadProject { operand, field } => {
            let variant = field.variant();
            out.push_str(&format!(
                "{pad}VariantPayloadProject {} v{} f{}\n",
                module.enums[variant.enum_id()].name,
                variant.variant_index(),
                field.field_index()
            ));
            dump_expr(module, locals, operand, indent + 1, out);
        }
    }
}

pub(super) fn dump_call(
    module: &Module,
    locals: &Arena<Local>,
    call: &Call,
    destination: Option<LocalId>,
    indent: usize,
    out: &mut String,
) {
    let pad = "  ".repeat(indent);
    let callee = match &call.target.callee {
        Callee::User(id) => format!("@{}", module.functions[*id].symbol),
        Callee::Monomorphized(id) => format!("@{}", module.meta.instances[*id].symbol),
        Callee::Extern(id) => format!(
            "extern{} @{}",
            id.into_raw(),
            module.extern_functions[*id].native_symbol
        ),
        Callee::CoroutineSuspend { register } => format!(
            "@coroutine_suspend[register=@{}]",
            module.meta.instances[*register].symbol
        ),
        Callee::Closure(function_type) => {
            format!(
                "<closure:function_type{}>",
                function_type.into_raw().into_u32()
            )
        }
        Callee::FunctionBridge(function_type) => format!(
            "<function_bridge:function_type{}>",
            function_type.into_raw().into_u32()
        ),
        Callee::Runtime(function) => format!("@{}", function.symbol()),
    };
    let kind = match &call.target.kind {
        CallKind::Direct => "direct".to_string(),
        CallKind::Virtual { slot } => format!("virtual[{slot}]"),
        CallKind::Interface { interface, slot } => {
            format!("interface {}[{slot}]", module.interfaces[*interface].name)
        }
        CallKind::Closure { function_type } => {
            format!(
                "closure[function_type{}]",
                function_type.into_raw().into_u32()
            )
        }
        CallKind::FunctionBridge { function_type } => format!(
            "function_bridge[function_type{}]",
            function_type.into_raw().into_u32()
        ),
    };
    match destination {
        Some(local) => out.push_str(&format!(
            "{pad}call {}: {} = {callee} {kind}\n",
            locals[local].name,
            type_name(module, &locals[local].ty)
        )),
        None => out.push_str(&format!("{pad}call {callee} {kind}\n")),
    }
    for arg in &call.args {
        dump_expr(module, locals, arg, indent + 1, out);
    }
}

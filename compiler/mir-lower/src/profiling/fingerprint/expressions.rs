use super::*;

impl Writer<'_> {
    pub(super) fn expr(&mut self, expression: &mir::Expr) {
        self.ty(&expression.ty);
        use mir::ExprKind as E;
        match &expression.kind {
            E::MaybeUninit { wrapper, operation } => {
                self.tag("maybe-uninit");
                self.ty(&mir::Type::Struct(*wrapper));
                self.scalar(&operation.map(|_| ()));
                if let Some(value) = operation.operand() {
                    self.expr(value);
                }
            }
            E::Context(operation) => {
                self.tag("context");
                self.scalar(&operation.map(|_| ()));
                for value in operation.operands() {
                    self.expr(value);
                }
            }
            E::DataBorrow(operation) => {
                self.tag("data-borrow");
                self.tag(operation.kind.name());
                match operation.kind {
                    mir::DataBorrowOperationKind::DataPointer(source)
                    | mir::DataBorrowOperationKind::Length(source) => match source {
                        mir::BorrowDataSource::Array(id) => {
                            self.tag("array");
                            self.ty(&mir::Type::Class(id));
                        }
                        mir::BorrowDataSource::String => self.tag("string"),
                    },
                    mir::DataBorrowOperationKind::PushPinFrame
                    | mir::DataBorrowOperationKind::PopPinFrame => {}
                }
                self.expr(&operation.operand);
            }
            E::ArrayConst(value) => {
                self.tag("array-constant");
                self.immortal(*value);
            }
            E::StringConst(value) => {
                self.tag("string-constant");
                self.immortal(*value);
            }
            E::StringConcat(parts) => {
                self.tag("string-concat");
                self.number(parts.len() as u64);
                for part in parts {
                    match part {
                        mir::StringConcatPart::Value(value) => {
                            self.tag("value");
                            self.expr(value);
                        }
                        mir::StringConcatPart::Literal(value) => {
                            self.tag("literal");
                            self.tag(value);
                        }
                    }
                }
            }
            E::StringCopy(operation) => {
                self.tag("string-copy");
                self.scalar(&operation.map(|_| ()));
                for value in operation.operands() {
                    self.expr(value);
                }
            }
            E::StringByte(operation) => {
                self.tag("string-byte");
                self.scalar(&operation.map(|_| ()));
                for value in operation.operands() {
                    self.expr(value);
                }
            }
            E::IntegerLiteral(value) => {
                self.tag("integer");
                self.scalar(value);
            }
            E::MachineScalarLiteral(value) => {
                self.tag("machine-scalar");
                self.scalar(value);
            }
            E::BoolLiteral(value) => {
                self.tag("boolean");
                self.number(u64::from(*value));
            }
            E::CharLiteral(value) => {
                self.tag("character");
                self.number(u64::from(*value));
            }
            E::FloatLiteral(value) => {
                self.tag("float");
                self.scalar(value);
            }
            E::FloatUnary {
                kind,
                operation,
                operand,
            } => {
                self.tag("float-unary");
                self.scalar(&(kind, operation));
                self.expr(operand);
            }
            E::FloatBinary {
                kind,
                operation,
                lhs,
                rhs,
            } => {
                self.tag("float-binary");
                self.scalar(&(kind, operation));
                self.expr(lhs);
                self.expr(rhs);
            }
            E::FloatConversion {
                conversion,
                operand,
            } => {
                self.tag("float-convert");
                self.scalar(conversion);
                self.expr(operand);
            }
            E::CharCode(value) => {
                self.tag("char-code");
                self.expr(value);
            }
            E::CharFromCodeUnchecked(value) => {
                self.tag("char-from-code");
                self.expr(value);
            }
            E::UnitLiteral => self.tag("unit"),
            E::TupleLiteral(values) => {
                self.tag("tuple");
                self.expressions(values);
            }
            E::StructInit { struct_id, args } => {
                self.tag("struct-init");
                self.ty(&mir::Type::Struct(*struct_id));
                self.expressions(args);
            }
            E::StructConstruct { struct_id, fields } => {
                self.tag("struct-construct");
                self.ty(&mir::Type::Struct(*struct_id));
                self.expressions(fields);
            }
            E::ClassAlloc { class_id } => {
                self.tag("class-alloc");
                self.ty(&mir::Type::Class(*class_id));
            }
            E::ClosureAlloc { class, captures } => {
                self.tag("closure-alloc");
                self.closure(*class);
                self.number(captures.len() as u64);
                for capture in captures {
                    self.number(u64::from(capture.field()));
                    self.expr(capture.value());
                }
            }
            E::ClosureCapture {
                closure,
                class,
                index,
            } => {
                self.tag("closure-capture");
                self.closure(*class);
                self.number(u64::from(*index));
                self.expr(closure);
            }
            E::Local(local) => {
                self.tag("local");
                self.local(*local);
            }
            E::GlobalRead(global) => {
                self.tag("global");
                self.global(*global);
            }
            E::InitializationUnitAddress(unit) => {
                self.tag("initialization-unit");
                self.wire(&self.module.initialization_units[*unit].identity.id());
            }
            E::PtrFromNonZeroULong { operand, pointee } => {
                self.tag("pointer-from-integer");
                self.ty(pointee);
                self.expr(operand);
            }
            E::PtrToULong(operand) => {
                self.tag("pointer-to-integer");
                self.expr(operand);
            }
            E::NativeFunctionPointerCast {
                operand,
                target_mode,
            } => {
                self.tag("native-pointer-cast");
                self.scalar(target_mode);
                self.expr(operand);
            }
            E::PtrCast { operand, pointee } => {
                self.tag("pointer-cast");
                self.ty(pointee);
                self.expr(operand);
            }
            E::PtrLoad {
                pointer,
                pointee,
                offset,
            } => {
                self.tag("pointer-load");
                self.ty(pointee);
                self.expr(pointer);
                self.optional_expr(offset.as_deref());
            }
            E::PtrStore {
                pointer,
                pointee,
                offset,
                value,
            } => {
                self.tag("pointer-store");
                self.ty(pointee);
                self.expr(pointer);
                self.optional_expr(offset.as_deref());
                self.expr(value);
            }
            E::PtrOffset {
                pointer,
                pointee,
                offset,
                subtract,
            } => {
                self.tag("pointer-offset");
                self.ty(pointee);
                self.expr(pointer);
                self.expr(offset);
                self.number(u64::from(*subtract));
            }
            E::AddressOf { local, pointee } => {
                self.tag("address");
                self.local(*local);
                self.ty(pointee);
            }
            E::GlobalAddress { global, pointee } => {
                self.tag("global-address");
                self.global(*global);
                self.ty(pointee);
            }
            E::SizeOf(ty) => {
                self.tag("size-of");
                self.ty(ty);
            }
            E::AlignOf(ty) => {
                self.tag("align-of");
                self.ty(ty);
            }
            E::FunctionAddress { callback } => {
                self.tag("function-address");
                self.wire(
                    &self.module.callback_bridges[*callback]
                        .identity()
                        .callable_record()
                        .id(),
                );
            }
            E::ForeignCallbackRegister { bridge, closure } => {
                self.tag("callback-register");
                self.wire(&self.module.foreign_callback_bridges[*bridge].application());
                self.expr(closure);
            }
            E::ForeignCallbackOperation {
                operation,
                callback,
            } => {
                self.tag("callback-operation");
                self.tag(operation.name());
                let family = &self.module.foreign_callback_families[operation.family()];
                self.ty(&mir::Type::Struct(family.callback));
                self.expr(callback);
            }
            E::CaughtException { state } => {
                self.tag("caught");
                self.exception(*state);
            }
            E::Retype { operand, ty } => {
                self.tag("retype");
                self.ty(ty);
                self.expr(operand);
            }
            E::FieldAccess { receiver, index } => {
                self.tag("field");
                self.expr(receiver);
                self.number(u64::from(*index));
            }
            E::ReleaseFieldLoad { class, index } => {
                self.tag("release-field");
                self.ty(&mir::Type::Class(*class));
                self.number(u64::from(*index));
            }
            E::AtomicFieldLoad {
                kind,
                object,
                index,
            } => {
                self.tag("atomic-field-load");
                self.scalar(kind);
                self.expr(object);
                self.number(u64::from(*index));
            }
            E::AtomicFieldCompareExchange {
                kind,
                object,
                index,
                expected,
                replacement,
            } => {
                self.tag("atomic-field-cas");
                self.scalar(kind);
                self.expr(object);
                self.number(u64::from(*index));
                self.expr(expected);
                self.expr(replacement);
            }
            E::Box(value) => {
                self.tag("box");
                self.expr(value);
            }
            E::Unbox(value) => {
                self.tag("unbox");
                self.expr(value);
            }
            E::IsInstance { operand, check_ty } => {
                self.tag("is-instance");
                self.ty(check_ty);
                self.expr(operand);
            }
            E::ArrayAllocate { array_type, count } => {
                self.tag("array-allocate");
                self.ty(&mir::Type::Class(*array_type));
                self.expr(count);
            }
            E::ArrayLiteral {
                array_type,
                elements,
            } => {
                self.tag("array-literal");
                self.ty(&mir::Type::Class(*array_type));
                self.expressions(elements);
            }
            E::ArrayAssembly { array_type, parts } => {
                self.tag("array-assembly");
                self.ty(&mir::Type::Class(*array_type));
                self.number(parts.len() as u64);
                for part in parts {
                    match part {
                        mir::ArrayAssemblyPart::Element(value) => {
                            self.tag("element");
                            self.expr(value);
                        }
                        mir::ArrayAssemblyPart::CopyArray(value) => {
                            self.tag("copy");
                            self.expr(value);
                        }
                    }
                }
            }
            E::ArrayGet {
                array_type,
                array,
                index,
            } => {
                self.tag("array-get");
                self.ty(&mir::Type::Class(*array_type));
                self.expr(array);
                self.expr(index);
            }
            E::ArrayLen {
                array_type,
                operand,
            } => {
                self.tag("array-length");
                self.ty(&mir::Type::Class(*array_type));
                self.expr(operand);
            }
            E::ArrayClone {
                source_type,
                target_type,
                operand,
            } => {
                self.tag("array-clone");
                self.ty(&mir::Type::Class(*source_type));
                self.ty(&mir::Type::Class(*target_type));
                self.expr(operand);
            }
            E::AtomicNew(value) => {
                self.tag("atomic-new");
                self.expr(value);
            }
            E::Atomic(operation) => {
                self.tag("atomic");
                self.scalar(&operation.map(|_| ()));
                for value in operation.operands() {
                    self.expr(value);
                }
            }
            E::Binary { op, lhs, rhs } => {
                self.tag("binary");
                self.scalar(op);
                self.expr(lhs);
                self.expr(rhs);
            }
            E::Unary { op, operand } => {
                self.tag("unary");
                self.scalar(op);
                self.expr(operand);
            }
            E::IntegerUnary { operation, operand } => {
                self.tag("integer-unary");
                self.scalar(operation);
                self.expr(operand);
            }
            E::IntegerBinary {
                operation,
                lhs,
                rhs,
            } => {
                self.tag("integer-binary");
                self.scalar(operation);
                self.expr(lhs);
                self.expr(rhs);
            }
            E::SafeIntegerDivRem {
                operation,
                lhs,
                rhs,
            } => {
                self.tag("integer-div-rem");
                self.scalar(operation);
                self.expr(lhs);
                self.expr(rhs);
            }
            E::IntegerCompare {
                operation,
                lhs,
                rhs,
            } => {
                self.tag("integer-compare");
                self.scalar(operation);
                self.expr(lhs);
                self.expr(rhs);
            }
            E::IntegerCompareTo {
                operation,
                lhs,
                rhs,
            } => {
                self.tag("integer-compare-to");
                self.scalar(operation);
                self.expr(lhs);
                self.expr(rhs);
            }
            E::IntegerShift {
                operation,
                value,
                count,
            } => {
                self.tag("integer-shift");
                self.scalar(operation);
                self.expr(value);
                self.expr(count);
            }
            E::IntegerConversion {
                conversion,
                operand,
            } => {
                self.tag("integer-convert");
                self.scalar(conversion);
                self.expr(operand);
            }
            E::VariantConstruct { variant, fields } => {
                self.tag("variant");
                self.variant(*variant);
                self.expressions(fields);
            }
            E::EnumTag(value) => {
                self.tag("enum-tag");
                self.expr(value);
            }
            E::EnumField {
                operand,
                variant,
                index,
            } => {
                self.tag("enum-field");
                self.expr(operand);
                self.number(u64::from(*variant));
                self.number(u64::from(*index));
            }
            E::VariantTest { operand, variant } => {
                self.tag("variant-test");
                self.variant(*variant);
                self.expr(operand);
            }
            E::VariantPayloadProject { operand, field } => {
                self.tag("variant-project");
                self.variant(field.variant());
                self.number(u64::from(field.field_index()));
                self.expr(operand);
            }
        }
    }

    fn variant(&mut self, variant: mir::MirVariantRef) {
        let ty = variant
            .enum_type(&self.module.enums)
            .expect("MIR variants retain their exact enum type");
        self.ty(&ty);
        self.number(u64::from(variant.variant_index()));
    }

    fn expressions(&mut self, values: &[mir::Expr]) {
        self.number(values.len() as u64);
        for value in values {
            self.expr(value);
        }
    }

    fn optional_expr(&mut self, value: Option<&mir::Expr>) {
        self.number(u64::from(value.is_some()));
        if let Some(value) = value {
            self.expr(value);
        }
    }
}

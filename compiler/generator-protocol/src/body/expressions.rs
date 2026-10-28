use super::*;

impl Serialize for BodyJson<'_, hir::DefaultExpressionV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {
            "operation": self.child(self.node.kind()), "resultType": self.child(self.node.result_type()),
            "origin": QueryJson(self.node.definition_origin()), "evaluationOrigin": QueryJson(self.node.evaluation_origin()),
        })
    }
}

impl Serialize for BodyJson<'_, hir::DefaultExpressionKindV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use hir::DefaultExpressionKindV1::*;
        match self.node {
            ContextLookup {
                declaration,
                parameter,
                diagnostic,
            } => object!(serializer, {
                "kind": "contextLookup", "declaration": CallableDeclaration::from(*declaration),
                "parameter": parameter.0, "diagnostic": QueryJson(diagnostic),
            }),
            ReleaseFieldLoad {
                owner_type,
                declaration,
            } => {
                object!(serializer, {"kind": "releaseFieldLoad", "ownerType": self.child(owner_type), "declaration": Reference::from(*declaration)})
            }
            ByteArrayLiteral(bytes) => {
                object!(serializer, {"kind": "byteArrayLiteral", "bytes": bytes})
            }
            StringLiteral { value, owner } => {
                object!(serializer, {"kind": "stringLiteral", "value": value, "owner": QueryJson(owner)})
            }
            IntegerLiteral(value) => {
                object!(serializer, {"kind": "integerLiteral", "value": QueryJson(value)})
            }
            BooleanLiteral(value) => {
                object!(serializer, {"kind": "booleanLiteral", "value": value.value()})
            }
            CharLiteral(value) => {
                object!(serializer, {"kind": "charLiteral", "value": value.value()})
            }
            FloatLiteral(value) => {
                object!(serializer, {"kind": "floatLiteral", "value": QueryJson(value)})
            }
            FloatUnary {
                kind,
                operation,
                operand,
            } => {
                object!(serializer, {"kind": "floatUnary", "floatKind": QueryJson(kind), "operator": QueryJson(operation), "operand": self.child(operand)})
            }
            FloatBinary {
                kind,
                operation,
                lhs,
                rhs,
            } => {
                object!(serializer, {"kind": "floatBinary", "floatKind": QueryJson(kind), "operator": QueryJson(operation), "lhs": self.child(lhs), "rhs": self.child(rhs)})
            }
            FloatConversion {
                conversion,
                operand,
            } => {
                object!(serializer, {"kind": "floatConversion", "conversion": QueryJson(conversion), "operand": self.child(operand)})
            }
            Unreachable => object!(serializer, {"kind": "unreachable"}),
            UnitLiteral => object!(serializer, {"kind": "unitLiteral"}),
            AnnotationKey(id) => {
                object!(serializer, {"kind": "annotationKey", "annotation": Reference::from(*id)})
            }
            TupleLiteral(elements) => {
                object!(serializer, {"kind": "tupleLiteral", "elements": self.child(elements)})
            }
            StructInit {
                constructor,
                arguments,
            } => {
                object!(serializer, {"kind": "structInit", "constructor": self.child(constructor), "arguments": self.child(arguments)})
            }
            StructConstruct { owner_type, fields } => {
                object!(serializer, {"kind": "structConstruct", "ownerType": self.child(owner_type), "fields": self.child(fields)})
            }
            ClassInit {
                constructor,
                arguments,
            } => {
                object!(serializer, {"kind": "classInit", "constructor": self.child(constructor), "arguments": self.child(arguments)})
            }
            VariantConstruct { variant, arguments } => {
                object!(serializer, {"kind": "variantConstruct", "variant": self.child(variant), "arguments": self.child(arguments)})
            }
            VariantTest { operand, variant } => {
                object!(serializer, {"kind": "variantTest", "operand": self.child(operand), "variant": self.child(variant)})
            }
            VariantPayloadProject { operand, field } => {
                object!(serializer, {"kind": "variantPayloadProject", "operand": self.child(operand), "field": self.child(field)})
            }
            Local(selector) => {
                object!(serializer, {"kind": "local", "selector": QueryJson(selector)})
            }
            Capture(index) => object!(serializer, {"kind": "capture", "index": index}),
            GlobalRead(id) => {
                object!(serializer, {"kind": "globalRead", "property": Reference::from(*id)})
            }
            GenericDelegateStorageRead(delegate) => {
                object!(serializer, {"kind": "genericDelegateStorageRead", "delegate": self.child(delegate)})
            }
            SingletonValue(id) => {
                object!(serializer, {"kind": "singletonValue", "object": Reference::from(*id)})
            }
            CompanionValue => object!(serializer, {"kind": "companionValue"}),
            Lambda(callable) => {
                object!(serializer, {"kind": "lambda", "callable": self.child(callable)})
            }
            AnonymousFunction(callable) => {
                object!(serializer, {"kind": "anonymousFunction", "callable": self.child(callable)})
            }
            CallableReference(callable) => {
                object!(serializer, {"kind": "callableReference", "callable": self.child(callable)})
            }
            FunctionCoercion {
                source,
                source_function_type,
                target_function_type,
            } => {
                object!(serializer, {"kind": "functionCoercion", "source": self.child(source), "sourceFunctionType": self.child(source_function_type), "targetFunctionType": self.child(target_function_type)})
            }
            PtrFromNonZeroULong(value) => self.unary(serializer, "ptrFromNonZeroULong", value),
            PtrToULong(value) => self.unary(serializer, "ptrToULong", value),
            CharCode(value) => self.unary(serializer, "charCode", value),
            CharFromCodeUnchecked(value) => self.unary(serializer, "charFromCodeUnchecked", value),
            PtrCast(value) => self.unary(serializer, "ptrCast", value),
            PtrLoad { pointer, offset } => {
                object!(serializer, {"kind": "ptrLoad", "pointer": self.child(pointer), "offset": self.child(offset)})
            }
            PtrStore {
                pointer,
                offset,
                value,
            } => {
                object!(serializer, {"kind": "ptrStore", "pointer": self.child(pointer), "offset": self.child(offset), "value": self.child(value)})
            }
            PtrOffset {
                pointer,
                offset,
                subtract,
            } => {
                object!(serializer, {"kind": "ptrOffset", "pointer": self.child(pointer), "offset": self.child(offset), "subtract": subtract.value()})
            }
            AddressOf(place) => {
                object!(serializer, {"kind": "addressOf", "place": QueryJson(place)})
            }
            SizeOf(value_type) => {
                object!(serializer, {"kind": "sizeOf", "valueType": self.child(value_type)})
            }
            AlignOf(value_type) => {
                object!(serializer, {"kind": "alignOf", "valueType": self.child(value_type)})
            }
            FunctionAddress(declaration) => {
                object!(serializer, {"kind": "functionAddress", "declaration": CallableDeclaration::from(*declaration)})
            }
            ForeignCallbackRegister {
                registration,
                closure,
            } => {
                object!(serializer, {"kind": "foreignCallbackRegister", "registration": Reference::from(*registration), "closure": self.child(closure)})
            }
            ForeignCallbackOperation {
                operation,
                callback,
            } => {
                object!(serializer, {"kind": "foreignCallbackOperation", "operator": QueryJson(operation), "callback": self.child(callback)})
            }
            FieldAccess { receiver, field } => {
                object!(serializer, {"kind": "fieldAccess", "receiver": self.child(receiver), "field": self.child(field)})
            }
            MethodCall {
                receiver,
                callee,
                arguments,
            } => {
                object!(serializer, {"kind": "methodCall", "receiver": self.child(receiver), "callee": self.child(callee), "arguments": self.child(arguments)})
            }
            DirectSuperMethodCall {
                receiver,
                callee,
                arguments,
            } => {
                object!(serializer, {"kind": "directSuperMethodCall", "receiver": self.child(receiver), "callee": self.child(callee), "arguments": self.child(arguments)})
            }
            Box(value) => self.unary(serializer, "box", value),
            Unbox(value) => self.unary(serializer, "unbox", value),
            Upcast(value) => self.unary(serializer, "upcast", value),
            IsInstance {
                operand,
                checked_type,
            } => {
                object!(serializer, {"kind": "isInstance", "operand": self.child(operand), "checkedType": self.child(checked_type)})
            }
            Cast {
                operand,
                checked_type,
                optional,
            } => {
                object!(serializer, {"kind": "cast", "operand": self.child(operand), "checkedType": self.child(checked_type), "optional": optional.value()})
            }
            ArrayLiteral(elements) => {
                object!(serializer, {"kind": "arrayLiteral", "elements": self.child(elements)})
            }
            ArrayGenerate { count, initializer } => {
                object!(serializer, {"kind": "arrayGenerate", "count": self.child(count), "initializer": self.child(initializer)})
            }
            ArrayAssembly(assembly) => {
                object!(serializer, {"kind": "arrayAssembly", "assembly": self.child(assembly)})
            }
            Index {
                access,
                receiver,
                index,
            } => {
                object!(serializer, {"kind": "index", "access": QueryJson(access), "receiver": self.child(receiver), "index": self.child(index)})
            }
            ArraySet {
                access,
                receiver,
                index,
                value,
            } => {
                object!(serializer, {"kind": "arraySet", "access": QueryJson(access), "receiver": self.child(receiver), "index": self.child(index), "value": self.child(value)})
            }
            ArrayLen(value) => self.unary(serializer, "arrayLen", value),
            ArrayClone(value) => self.unary(serializer, "arrayClone", value),
            AtomicNew(value) => self.unary(serializer, "atomicNew", value),
            Atomic(expression) => {
                object!(serializer, {"kind": "atomic", "expression": self.child(expression)})
            }
            MaybeUninit(operation) => {
                object!(serializer, {"kind": "maybeUninit", "operation": self.child(operation)})
            }
            Call {
                callee,
                arguments,
                receiver,
            } => {
                object!(serializer, {"kind": "call", "callee": self.child(callee), "arguments": self.child(arguments), "receiver": self.child(receiver)})
            }
            LocalFunctionCall {
                declaration,
                callee,
                captures,
                arguments,
            } => {
                object!(serializer, {"kind": "localFunctionCall", "declaration": CallableDeclaration::from(*declaration), "callee": self.child(callee), "captures": self.child(captures), "arguments": self.child(arguments)})
            }
            CallableCall {
                callee,
                function_type,
                arguments,
            } => {
                object!(serializer, {"kind": "callableCall", "callee": self.child(callee), "functionType": self.child(function_type), "arguments": self.child(arguments)})
            }
            PrimitiveBinary { kind, lhs, rhs } => {
                object!(serializer, {"kind": "primitiveBinary", "operator": QueryJson(kind), "lhs": self.child(lhs), "rhs": self.child(rhs)})
            }
            PrimitiveUnary { kind, operand } => {
                object!(serializer, {"kind": "primitiveUnary", "operator": QueryJson(kind), "operand": self.child(operand)})
            }
            IntegerOperation(operation) => {
                object!(serializer, {"kind": "integerOperation", "operation": self.child(operation)})
            }
            IntegerConversion {
                source_kind,
                target_kind,
                operand,
            } => {
                object!(serializer, {"kind": "integerConversion", "sourceKind": QueryJson(source_kind), "targetKind": QueryJson(target_kind), "operand": self.child(operand)})
            }
            Binary { operator, lhs, rhs } => {
                object!(serializer, {"kind": "binary", "operator": QueryJson(operator), "lhs": self.child(lhs), "rhs": self.child(rhs)})
            }
            Unary { operator, operand } => {
                object!(serializer, {"kind": "unary", "operator": QueryJson(operator), "operand": self.child(operand)})
            }
            SomeWrap(value) => self.unary(serializer, "someWrap", value),
            NoneLiteral => object!(serializer, {"kind": "noneLiteral"}),
            IsSome(value) => self.unary(serializer, "isSome", value),
            Unwrap {
                operand,
                trap_on_none,
            } => {
                object!(serializer, {"kind": "unwrap", "operand": self.child(operand), "trapOnNone": trap_on_none.value()})
            }
        }
    }
}

impl BodyJson<'_, hir::DefaultExpressionKindV1> {
    fn unary<S: Serializer>(
        &self,
        serializer: S,
        kind: &str,
        operand: &hir::DefaultExpressionV1,
    ) -> Result<S::Ok, S::Error> {
        object!(serializer, {"kind": kind, "operand": self.child(operand)})
    }
}

impl Serialize for BodyJson<'_, hir::OptionalDefaultExpressionV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node.as_ref() {
            Some(value) => self.child(value).serialize(serializer),
            None => serializer.serialize_none(),
        }
    }
}

impl Serialize for BodyJson<'_, hir::DefaultArrayAssemblyV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"elementType": self.child(self.node.element_type()), "parts": self.child(self.node.parts()), "resultType": self.child(self.node.result_type())})
    }
}

impl Serialize for BodyJson<'_, hir::DefaultArrayAssemblyPartV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.node {
            hir::DefaultArrayAssemblyPartV1::Element(value) => {
                object!(serializer, {"kind": "element", "value": self.child(value)})
            }
            hir::DefaultArrayAssemblyPartV1::CopyArray(value) => {
                object!(serializer, {"kind": "spread", "value": self.child(value)})
            }
        }
    }
}

impl Serialize for QueryJson<'_, hir::ContextDiagnostic> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        object!(serializer, {"declaration": self.0.declaration, "label": QueryJson(&self.0.label)})
    }
}

impl Serialize for QueryJson<'_, hir::DefaultStringOwnerV1> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            hir::DefaultStringOwnerV1::CurrentInstantiation => {
                object!(serializer, {"kind": "currentInstantiation"})
            }
            hir::DefaultStringOwnerV1::Property(property) => {
                object!(serializer, {"kind": "property", "property": Reference::from(*property)})
            }
            hir::DefaultStringOwnerV1::PropertyElement { property, ordinal } => {
                object!(serializer, {"kind": "propertyElement", "property": Reference::from(*property), "ordinal": ordinal})
            }
        }
    }
}

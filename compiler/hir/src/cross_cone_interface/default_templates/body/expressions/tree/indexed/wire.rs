use scoop_wire::{Encoder, WireEncode};

use super::{
    IndexedDefaultArrayAssemblyPartV1, IndexedDefaultArrayAssemblyV1,
    IndexedDefaultExpressionKindV1, IndexedDefaultExpressionV1, IndexedDefaultIntegerArgumentsV1,
    IndexedOptionalDefaultExpressionV1,
};

impl WireEncode for IndexedDefaultExpressionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.expression.result_type.encode(encoder)?;
        encoder.field(3)?;
        self.expression.definition_origin.encode(encoder)?;
        encoder.field(4)?;
        self.expression.evaluation_origin.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultExpressionKindV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ContextLookup {
                declaration,
                parameter,
                diagnostic,
            } => encode_three(
                encoder,
                66,
                *declaration,
                &U32Wire(parameter.0),
                *diagnostic,
            ),
            Self::StringLiteral { value, owner } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                encoder.text(value)?;
                encoder.field(2)?;
                owner.encode(encoder)
            }
            Self::IntegerLiteral(value) => encode_one(encoder, 2, *value),
            Self::BooleanLiteral(value) => encode_one(encoder, 3, value),
            Self::CharLiteral(value) => encode_one(encoder, 63, value),
            Self::FloatLiteral(value) => encode_one(encoder, 67, value),
            Self::FloatUnary {
                kind,
                operation,
                operand,
            } => encode_three(encoder, 68, kind, operation, operand.as_ref()),
            Self::FloatBinary {
                kind,
                operation,
                lhs,
                rhs,
            } => encode_four(encoder, 69, kind, operation, lhs.as_ref(), rhs.as_ref()),
            Self::FloatConversion {
                conversion,
                operand,
            } => encode_two(encoder, 70, conversion, operand.as_ref()),
            Self::UnitLiteral => encode_empty(encoder, 4),
            Self::TupleLiteral(elements) => encode_sequence_variant(encoder, 5, elements),
            Self::StructInit {
                constructor,
                arguments,
            } => encode_two(encoder, 6, *constructor, &WireSequence(arguments)),
            Self::StructConstruct { owner_type, fields } => {
                encode_two(encoder, 7, *owner_type, &WireSequence(fields))
            }
            Self::ClassInit {
                constructor,
                arguments,
            } => encode_two(encoder, 8, *constructor, &WireSequence(arguments)),
            Self::VariantConstruct { variant, arguments } => {
                encode_two(encoder, 9, *variant, &WireSequence(arguments))
            }
            Self::VariantTest { operand, variant } => {
                encode_two(encoder, 10, operand.as_ref(), *variant)
            }
            Self::VariantPayloadProject { operand, field } => {
                encode_two(encoder, 11, operand.as_ref(), *field)
            }
            Self::Local(local_index) => encode_one(encoder, 12, &U32Wire(*local_index)),
            Self::Capture(index) => encode_one(encoder, 59, &U32Wire(*index)),
            Self::GlobalRead(property) => encode_one(encoder, 13, *property),
            Self::GenericDelegateStorageRead(reference) => encode_one(encoder, 60, *reference),
            Self::SingletonValue(value) => encode_one(encoder, 14, *value),
            Self::Lambda(lambda) => encode_one(encoder, 15, lambda),
            Self::AnonymousFunction(function) => encode_one(encoder, 16, function),
            Self::CallableReference(reference) => encode_one(encoder, 17, reference),
            Self::FunctionCoercion {
                source,
                source_function_type,
                target_function_type,
            } => encode_three(
                encoder,
                18,
                source.as_ref(),
                *source_function_type,
                *target_function_type,
            ),
            Self::PtrFromNonZeroULong(operand) => encode_one(encoder, 19, operand.as_ref()),
            Self::PtrToULong(operand) => encode_one(encoder, 20, operand.as_ref()),
            Self::CharCode(operand) => encode_one(encoder, 64, operand.as_ref()),
            Self::CharFromCodeUnchecked(operand) => encode_one(encoder, 65, operand.as_ref()),
            Self::PtrCast(operand) => encode_one(encoder, 21, operand.as_ref()),
            Self::PtrLoad { pointer, offset } => encode_two(encoder, 22, pointer.as_ref(), offset),
            Self::PtrStore {
                pointer,
                offset,
                value,
            } => encode_three(encoder, 23, pointer.as_ref(), offset, value.as_ref()),
            Self::PtrOffset {
                pointer,
                offset,
                subtract,
            } => encode_three(encoder, 24, pointer.as_ref(), offset.as_ref(), subtract),
            Self::AddressOf(place) => encode_one(encoder, 25, place),
            Self::SizeOf(queried_type) => encode_one(encoder, 26, *queried_type),
            Self::AlignOf(queried_type) => encode_one(encoder, 27, *queried_type),
            Self::FunctionAddress(declaration) => encode_one(encoder, 28, *declaration),
            Self::ForeignCallbackRegister {
                registration,
                closure,
            } => encode_two(encoder, 29, *registration, closure.as_ref()),
            Self::ForeignCallbackOperation {
                operation,
                callback,
            } => encode_two(encoder, 30, operation, callback.as_ref()),
            Self::ReleaseFieldLoad {
                owner_type,
                declaration,
            } => encode_two(encoder, 61, *owner_type, *declaration),
            Self::FieldAccess { receiver, field } => {
                encode_two(encoder, 31, receiver.as_ref(), *field)
            }
            Self::MethodCall {
                receiver,
                callee,
                arguments,
            } => encode_three(
                encoder,
                32,
                receiver.as_ref(),
                *callee,
                &WireSequence(arguments),
            ),
            Self::DirectSuperMethodCall {
                receiver,
                callee,
                arguments,
            } => encode_three(
                encoder,
                33,
                receiver.as_ref(),
                *callee,
                &WireSequence(arguments),
            ),
            Self::Box(operand) => encode_one(encoder, 34, operand.as_ref()),
            Self::Unbox(operand) => encode_one(encoder, 35, operand.as_ref()),
            Self::ReferenceUpcast(operand) => encode_one(encoder, 58, operand.as_ref()),
            Self::IsInstance {
                operand,
                checked_type,
            } => encode_two(encoder, 36, operand.as_ref(), *checked_type),
            Self::Cast {
                operand,
                checked_type,
                optional,
            } => encode_three(encoder, 37, operand.as_ref(), *checked_type, optional),
            Self::ArrayLiteral(elements) => encode_sequence_variant(encoder, 38, elements),
            Self::ArrayGenerate { count, initializer } => {
                encode_two(encoder, 62, count.as_ref(), initializer.as_ref())
            }
            Self::ArrayAssembly(assembly) => encode_one(encoder, 39, assembly),
            Self::Index {
                access,
                receiver,
                index,
            } => encode_three(encoder, 40, access, receiver.as_ref(), index.as_ref()),
            Self::ArraySet {
                access,
                receiver,
                index,
                value,
            } => encode_four(
                encoder,
                41,
                access,
                receiver.as_ref(),
                index.as_ref(),
                value.as_ref(),
            ),
            Self::ArrayLen(operand) => encode_one(encoder, 42, operand.as_ref()),
            Self::ArrayClone(operand) => encode_one(encoder, 43, operand.as_ref()),
            Self::Call {
                callee,
                arguments,
                receiver,
            } => encode_three(encoder, 57, *callee, &WireSequence(arguments), *receiver),
            Self::LocalFunctionCall {
                declaration,
                callee,
                captures,
                arguments,
            } => encode_four(
                encoder,
                45,
                *declaration,
                *callee,
                &WireSequence(captures),
                &WireSequence(arguments),
            ),
            Self::CallableCall {
                callee,
                function_type,
                arguments,
            } => encode_three(
                encoder,
                46,
                callee.as_ref(),
                *function_type,
                &WireSequence(arguments),
            ),
            Self::PrimitiveBinary { kind, lhs, rhs } => {
                encode_three(encoder, 47, kind, lhs.as_ref(), rhs.as_ref())
            }
            Self::PrimitiveUnary { kind, operand } => {
                encode_two(encoder, 48, kind, operand.as_ref())
            }
            Self::IntegerOperation {
                operation,
                arguments,
            } => encode_two(encoder, 49, *operation, arguments),
            Self::IntegerConversion {
                source_kind,
                target_kind,
                operand,
            } => encode_three(encoder, 50, source_kind, target_kind, operand.as_ref()),
            Self::Binary { operator, lhs, rhs } => {
                encode_three(encoder, 51, operator, lhs.as_ref(), rhs.as_ref())
            }
            Self::Unary { operator, operand } => {
                encode_two(encoder, 52, operator, operand.as_ref())
            }
            Self::SomeWrap(operand) => encode_one(encoder, 53, operand.as_ref()),
            Self::NoneLiteral => encode_empty(encoder, 54),
            Self::IsSome(operand) => encode_one(encoder, 55, operand.as_ref()),
            Self::Unwrap {
                operand,
                trap_on_none,
            } => encode_two(encoder, 56, operand.as_ref(), trap_on_none),
        }
    }
}

impl WireEncode for IndexedOptionalDefaultExpressionV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty(encoder, 1),
            Self::Present(expression) => encode_one(encoder, 2, expression.as_ref()),
        }
    }
}

impl WireEncode for IndexedDefaultArrayAssemblyV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        self.element_type.encode(encoder)?;
        encoder.field(2)?;
        encode_sequence(encoder, &self.parts)?;
        encoder.field(3)?;
        self.result_type.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultArrayAssemblyPartV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Element(expression) => encode_one(encoder, 1, expression),
            Self::CopyArray(expression) => encode_one(encoder, 2, expression),
        }
    }
}

impl WireEncode for IndexedDefaultIntegerArgumentsV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unary(operand) => encode_one(encoder, 1, operand.as_ref()),
            Self::Binary { lhs, rhs } => encode_two(encoder, 2, lhs.as_ref(), rhs.as_ref()),
        }
    }
}

struct WireSequence<'a, T>(&'a [T]);

impl<T: WireEncode> WireEncode for WireSequence<'_, T> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encode_sequence(encoder, self.0)
    }
}

struct U32Wire(u32);

impl WireEncode for U32Wire {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(u64::from(self.0))
    }
}

fn encode_sequence_variant(
    encoder: &mut Encoder,
    tag: u64,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encode_one(encoder, tag, &WireSequence(values))
}

fn encode_sequence(
    encoder: &mut Encoder,
    values: &[impl WireEncode],
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.array(values.len() as u64)?;
    for value in values {
        value.encode(encoder)?;
    }
    Ok(())
}

fn encode_empty(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(1)?;
    encode_tag(encoder, tag)
}

fn encode_one(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)
}

fn encode_two(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(3)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)
}

fn encode_three(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
    third: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(4)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)?;
    encoder.field(3)?;
    third.encode(encoder)
}

fn encode_four(
    encoder: &mut Encoder,
    tag: u64,
    first: &impl WireEncode,
    second: &impl WireEncode,
    third: &impl WireEncode,
    fourth: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(5)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    first.encode(encoder)?;
    encoder.field(2)?;
    second.encode(encoder)?;
    encoder.field(3)?;
    third.encode(encoder)?;
    encoder.field(4)?;
    fourth.encode(encoder)
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

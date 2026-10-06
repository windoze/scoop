use scoop_identity::{DecodedCallableTemplateOrigin, DecodedPersistentId, DecodedSignatureTypeKey};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::super::super::DecodedDefaultCallableReferenceV1;
use super::{
    DecodedDefaultArrayAssemblyPartV1, DecodedDefaultArrayAssemblyV1,
    DecodedDefaultExpressionKindV1, DecodedDefaultExpressionV1, DecodedDefaultIntegerArgumentsV1,
    DecodedOptionalDefaultExpressionV1,
};
use crate::{
    CanonicalBooleanV1, CanonicalIntegerConstantV1, DecodedDefaultAnonymousFunctionV1,
    DecodedDefaultCallableDeclarationV1, DecodedDefaultCallableRefV1,
    DecodedDefaultConstructorRefV1, DecodedDefaultEnumVariantFieldRefV1,
    DecodedDefaultEnumVariantRefV1, DecodedDefaultFieldRefV1, DecodedDefaultIntegerOperationV1,
    DecodedDefaultLambdaV1, DecodedDefaultMethodCalleeV1, DecodedDefaultPlaceV1,
    DecodedDefaultStringOwnerV1, DecodedExportDefinitionSourceV1, DefaultArrayAccessKindV1,
    DefaultBinaryOperatorV1, DefaultForeignCallbackOperationV1, DefaultIntegerKindV1,
    DefaultPrimitiveBinaryKindV1, DefaultPrimitiveUnaryKindV1, DefaultUnaryOperatorV1,
};

impl WireEncode for DecodedDefaultExpressionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.kind.encode(encoder)?;
        encoder.field(2)?;
        self.result_type.encode(encoder)?;
        encoder.field(3)?;
        self.definition_origin.encode(encoder)?;
        encoder.field(4)?;
        self.evaluation_origin.encode(encoder)
    }
}

impl WireDecode for DecodedDefaultExpressionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            kind: decoder.field(1, DecodedDefaultExpressionKindV1::decode)?,
            result_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            definition_origin: decoder.field(3, DecodedExportDefinitionSourceV1::decode)?,
            evaluation_origin: decoder.field(4, scoop_identity::DecodedEvaluationOrigin::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultExpressionKindV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ContextLookup {
                declaration,
                parameter,
                diagnostic,
            } => encode_three(encoder, 66, declaration, &U32Wire(parameter.0), diagnostic),
            Self::StringLiteral { value, owner } => {
                encoder.map(3)?;
                encode_tag(encoder, 1)?;
                encoder.field(1)?;
                encoder.text(value)?;
                encoder.field(2)?;
                owner.encode(encoder)
            }
            Self::IntegerLiteral(value) => encode_one(encoder, 2, value),
            Self::BooleanLiteral(value) => encode_one(encoder, 3, value),
            Self::CharLiteral(value) => encode_one(encoder, 63, value),
            Self::FloatLiteral(value) => encode_one(encoder, 67, value),
            Self::UnitLiteral => encode_empty(encoder, 4),
            Self::TupleLiteral(elements) => encode_one(encoder, 5, &WireSequence(elements)),
            Self::StructInit {
                constructor,
                arguments,
            } => encode_two(encoder, 6, constructor, &WireSequence(arguments)),
            Self::StructConstruct { owner_type, fields } => {
                encode_two(encoder, 7, owner_type, &WireSequence(fields))
            }
            Self::ClassInit {
                constructor,
                arguments,
            } => encode_two(encoder, 8, constructor, &WireSequence(arguments)),
            Self::VariantConstruct { variant, arguments } => {
                encode_two(encoder, 9, variant, &WireSequence(arguments))
            }
            Self::VariantTest { operand, variant } => {
                encode_two(encoder, 10, operand.as_ref(), variant)
            }
            Self::VariantPayloadProject { operand, field } => {
                encode_two(encoder, 11, operand.as_ref(), field)
            }
            Self::Local(local_index) => encode_one(encoder, 12, &U32Wire(*local_index)),
            Self::Capture(index) => encode_one(encoder, 59, &U32Wire(*index)),
            Self::GlobalRead(property) => encode_one(encoder, 13, property),
            Self::GenericDelegateStorageRead(reference) => encode_one(encoder, 60, reference),
            Self::SingletonValue(value) => encode_one(encoder, 14, value),
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
                source_function_type,
                target_function_type,
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
            Self::SizeOf(queried_type) => encode_one(encoder, 26, queried_type),
            Self::AlignOf(queried_type) => encode_one(encoder, 27, queried_type),
            Self::FunctionAddress(declaration) => encode_one(encoder, 28, declaration),
            Self::ForeignCallbackRegister {
                registration,
                closure,
            } => encode_two(encoder, 29, registration, closure.as_ref()),
            Self::ForeignCallbackOperation {
                operation,
                callback,
            } => encode_two(encoder, 30, operation, callback.as_ref()),
            Self::ReleaseFieldLoad {
                owner_type,
                declaration,
            } => encode_two(encoder, 61, owner_type, declaration),
            Self::FieldAccess { receiver, field } => {
                encode_two(encoder, 31, receiver.as_ref(), field)
            }
            Self::MethodCall {
                receiver,
                callee,
                arguments,
            } => encode_three(
                encoder,
                32,
                receiver.as_ref(),
                callee,
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
                callee,
                &WireSequence(arguments),
            ),
            Self::Box(operand) => encode_one(encoder, 34, operand.as_ref()),
            Self::Unbox(operand) => encode_one(encoder, 35, operand.as_ref()),
            Self::ReferenceUpcast(operand) => encode_one(encoder, 58, operand.as_ref()),
            Self::IsInstance {
                operand,
                checked_type,
            } => encode_two(encoder, 36, operand.as_ref(), checked_type),
            Self::Cast {
                operand,
                checked_type,
                optional,
            } => encode_three(encoder, 37, operand.as_ref(), checked_type, optional),
            Self::ArrayLiteral(elements) => encode_one(encoder, 38, &WireSequence(elements)),
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
            } => encode_three(encoder, 57, callee, &WireSequence(arguments), receiver),
            Self::LocalFunctionCall {
                declaration,
                callee,
                captures,
                arguments,
            } => encode_four(
                encoder,
                45,
                declaration,
                callee,
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
                function_type,
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
            } => encode_two(encoder, 49, operation, arguments),
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

impl WireDecode for DecodedDefaultExpressionKindV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            66 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::ContextLookup {
                    declaration: decoder.field(1, DecodedDefaultCallableDeclarationV1::decode)?,
                    parameter: crate::ContextParameterIndex(decoder.field(2, Decoder::u32)?),
                    diagnostic: decoder.field(3, crate::ContextDiagnostic::decode)?,
                })
            }
            1 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::StringLiteral {
                    value: decoder.field(1, Decoder::owned_text)?,
                    owner: decoder.field(2, DecodedDefaultStringOwnerV1::decode)?,
                })
            }
            2 => decode_one(decoder, fields, CanonicalIntegerConstantV1::decode)
                .map(Self::IntegerLiteral),
            3 => decode_one(decoder, fields, CanonicalBooleanV1::decode).map(Self::BooleanLiteral),
            4 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::UnitLiteral)
            }
            5 => decode_expression_sequence(decoder, fields).map(Self::TupleLiteral),
            6 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::StructInit {
                    constructor: decoder.field(1, DecodedDefaultConstructorRefV1::decode)?,
                    arguments: decoder.field(2, decode_expression_array)?,
                })
            }
            7 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::StructConstruct {
                    owner_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    fields: decoder.field(2, decode_expression_array)?,
                })
            }
            8 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ClassInit {
                    constructor: decoder.field(1, DecodedDefaultConstructorRefV1::decode)?,
                    arguments: decoder.field(2, decode_expression_array)?,
                })
            }
            9 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::VariantConstruct {
                    variant: decoder.field(1, DecodedDefaultEnumVariantRefV1::decode)?,
                    arguments: decoder.field(2, decode_expression_array)?,
                })
            }
            10 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::VariantTest {
                    operand: decode_boxed_expression_field(decoder, 1)?,
                    variant: decoder.field(2, DecodedDefaultEnumVariantRefV1::decode)?,
                })
            }
            11 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::VariantPayloadProject {
                    operand: decode_boxed_expression_field(decoder, 1)?,
                    field: decoder.field(2, DecodedDefaultEnumVariantFieldRefV1::decode)?,
                })
            }
            12 => decode_one(decoder, fields, |decoder| decoder.u32()).map(Self::Local),
            61 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ReleaseFieldLoad {
                    owner_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
                    declaration: decoder.field(2, DecodedPersistentId::decode)?,
                })
            }
            59 => decode_one(decoder, fields, |decoder| decoder.u32()).map(Self::Capture),
            13 => decode_one(decoder, fields, DecodedPersistentId::decode).map(Self::GlobalRead),
            60 => decode_one(
                decoder,
                fields,
                crate::DecodedDefaultGenericDelegateReferenceV1::decode,
            )
            .map(Self::GenericDelegateStorageRead),
            14 => {
                decode_one(decoder, fields, DecodedPersistentId::decode).map(Self::SingletonValue)
            }
            15 => decode_one(decoder, fields, DecodedDefaultLambdaV1::decode).map(Self::Lambda),
            16 => decode_one(decoder, fields, DecodedDefaultAnonymousFunctionV1::decode)
                .map(Self::AnonymousFunction),
            17 => decode_one(decoder, fields, DecodedDefaultCallableReferenceV1::decode)
                .map(Self::CallableReference),
            18 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::FunctionCoercion {
                    source: decode_boxed_expression_field(decoder, 1)?,
                    source_function_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
                    target_function_type: decoder.field(3, DecodedSignatureTypeKey::decode)?,
                })
            }
            19 => decode_boxed_expression(decoder, fields).map(Self::PtrFromNonZeroULong),
            20 => decode_boxed_expression(decoder, fields).map(Self::PtrToULong),
            67 => {
                decode_one(decoder, fields, crate::HirFloatConstant::decode).map(Self::FloatLiteral)
            }
            63 => {
                decode_one(decoder, fields, crate::CanonicalCharV1::decode).map(Self::CharLiteral)
            }
            64 => decode_boxed_expression(decoder, fields).map(Self::CharCode),
            65 => decode_boxed_expression(decoder, fields).map(Self::CharFromCodeUnchecked),
            21 => decode_boxed_expression(decoder, fields).map(Self::PtrCast),
            22 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::PtrLoad {
                    pointer: decode_boxed_expression_field(decoder, 1)?,
                    offset: decoder.field(2, DecodedOptionalDefaultExpressionV1::decode)?,
                })
            }
            23 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::PtrStore {
                    pointer: decode_boxed_expression_field(decoder, 1)?,
                    offset: decoder.field(2, DecodedOptionalDefaultExpressionV1::decode)?,
                    value: decode_boxed_expression_field(decoder, 3)?,
                })
            }
            24 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::PtrOffset {
                    pointer: decode_boxed_expression_field(decoder, 1)?,
                    offset: decode_boxed_expression_field(decoder, 2)?,
                    subtract: decoder.field(3, CanonicalBooleanV1::decode)?,
                })
            }
            25 => decode_one(decoder, fields, DecodedDefaultPlaceV1::decode).map(Self::AddressOf),
            26 => decode_one(decoder, fields, DecodedSignatureTypeKey::decode).map(Self::SizeOf),
            27 => decode_one(decoder, fields, DecodedSignatureTypeKey::decode).map(Self::AlignOf),
            28 => decode_one(decoder, fields, DecodedDefaultCallableDeclarationV1::decode)
                .map(Self::FunctionAddress),
            29 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ForeignCallbackRegister {
                    registration: decoder.field(1, DecodedPersistentId::decode)?,
                    closure: decode_boxed_expression_field(decoder, 2)?,
                })
            }
            30 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ForeignCallbackOperation {
                    operation: decoder.field(1, DefaultForeignCallbackOperationV1::decode)?,
                    callback: decode_boxed_expression_field(decoder, 2)?,
                })
            }
            31 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::FieldAccess {
                    receiver: decode_boxed_expression_field(decoder, 1)?,
                    field: decoder.field(2, DecodedDefaultFieldRefV1::decode)?,
                })
            }
            32 => decode_method_call(decoder, fields, false),
            33 => decode_method_call(decoder, fields, true),
            34 => decode_boxed_expression(decoder, fields).map(Self::Box),
            35 => decode_boxed_expression(decoder, fields).map(Self::Unbox),
            58 => decode_boxed_expression(decoder, fields).map(Self::ReferenceUpcast),
            36 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::IsInstance {
                    operand: decode_boxed_expression_field(decoder, 1)?,
                    checked_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
                })
            }
            37 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Cast {
                    operand: decode_boxed_expression_field(decoder, 1)?,
                    checked_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
                    optional: decoder.field(3, CanonicalBooleanV1::decode)?,
                })
            }
            62 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::ArrayGenerate {
                    count: decode_boxed_expression_field(decoder, 1)?,
                    initializer: decode_boxed_expression_field(decoder, 2)?,
                })
            }
            38 => decode_expression_sequence(decoder, fields).map(Self::ArrayLiteral),
            39 => decode_one(decoder, fields, DecodedDefaultArrayAssemblyV1::decode)
                .map(Self::ArrayAssembly),
            40 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Index {
                    access: decoder.field(1, DefaultArrayAccessKindV1::decode)?,
                    receiver: decode_boxed_expression_field(decoder, 2)?,
                    index: decode_boxed_expression_field(decoder, 3)?,
                })
            }
            41 => {
                expect_sum_length(decoder, fields, 5)?;
                Ok(Self::ArraySet {
                    access: decoder.field(1, DefaultArrayAccessKindV1::decode)?,
                    receiver: decode_boxed_expression_field(decoder, 2)?,
                    index: decode_boxed_expression_field(decoder, 3)?,
                    value: decode_boxed_expression_field(decoder, 4)?,
                })
            }
            42 => decode_boxed_expression(decoder, fields).map(Self::ArrayLen),
            43 => decode_boxed_expression(decoder, fields).map(Self::ArrayClone),
            57 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Call {
                    callee: decoder.field(1, DecodedDefaultCallableRefV1::decode)?,
                    arguments: decoder.field(2, decode_expression_array)?,
                    receiver: decoder.field(3, crate::SourceCallReceiver::decode)?,
                })
            }
            45 => {
                expect_sum_length(decoder, fields, 5)?;
                Ok(Self::LocalFunctionCall {
                    declaration: decoder.field(1, DecodedCallableTemplateOrigin::decode)?,
                    callee: decoder.field(2, DecodedDefaultCallableRefV1::decode)?,
                    captures: decoder.field(3, decode_expression_array)?,
                    arguments: decoder.field(4, decode_expression_array)?,
                })
            }
            46 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::CallableCall {
                    callee: decode_boxed_expression_field(decoder, 1)?,
                    function_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
                    arguments: decoder.field(3, decode_expression_array)?,
                })
            }
            47 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::PrimitiveBinary {
                    kind: decoder.field(1, DefaultPrimitiveBinaryKindV1::decode)?,
                    lhs: decode_boxed_expression_field(decoder, 2)?,
                    rhs: decode_boxed_expression_field(decoder, 3)?,
                })
            }
            48 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::PrimitiveUnary {
                    kind: decoder.field(1, DefaultPrimitiveUnaryKindV1::decode)?,
                    operand: decode_boxed_expression_field(decoder, 2)?,
                })
            }
            49 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::IntegerOperation {
                    operation: decoder.field(1, DecodedDefaultIntegerOperationV1::decode)?,
                    arguments: decoder.field(2, DecodedDefaultIntegerArgumentsV1::decode)?,
                })
            }
            50 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::IntegerConversion {
                    source_kind: decoder.field(1, DefaultIntegerKindV1::decode)?,
                    target_kind: decoder.field(2, DefaultIntegerKindV1::decode)?,
                    operand: decode_boxed_expression_field(decoder, 3)?,
                })
            }
            51 => {
                expect_sum_length(decoder, fields, 4)?;
                Ok(Self::Binary {
                    operator: decoder.field(1, DefaultBinaryOperatorV1::decode)?,
                    lhs: decode_boxed_expression_field(decoder, 2)?,
                    rhs: decode_boxed_expression_field(decoder, 3)?,
                })
            }
            52 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Unary {
                    operator: decoder.field(1, DefaultUnaryOperatorV1::decode)?,
                    operand: decode_boxed_expression_field(decoder, 2)?,
                })
            }
            53 => decode_boxed_expression(decoder, fields).map(Self::SomeWrap),
            54 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::NoneLiteral)
            }
            55 => decode_boxed_expression(decoder, fields).map(Self::IsSome),
            56 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Unwrap {
                    operand: decode_boxed_expression_field(decoder, 1)?,
                    trap_on_none: decoder.field(2, CanonicalBooleanV1::decode)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for DecodedOptionalDefaultExpressionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty(encoder, 1),
            Self::Present(expression) => encode_one(encoder, 2, expression.as_ref()),
        }
    }
}

impl WireDecode for DecodedOptionalDefaultExpressionV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Absent)
            }
            2 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedDefaultExpressionV1::decode)
                    .map(Box::new)
                    .map(Self::Present)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for DecodedDefaultArrayAssemblyV1 {
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

impl WireDecode for DecodedDefaultArrayAssemblyV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(3)?;
        Ok(Self {
            element_type: decoder.field(1, DecodedSignatureTypeKey::decode)?,
            parts: decoder.field(2, |decoder| {
                decoder
                    .decode_array(|decoder, _| DecodedDefaultArrayAssemblyPartV1::decode(decoder))
            })?,
            result_type: decoder.field(3, DecodedSignatureTypeKey::decode)?,
        })
    }
}

impl WireEncode for DecodedDefaultArrayAssemblyPartV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Element(expression) => encode_one(encoder, 1, expression),
            Self::CopyArray(expression) => encode_one(encoder, 2, expression),
        }
    }
}

impl WireDecode for DecodedDefaultArrayAssemblyPartV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => decode_one(decoder, fields, DecodedDefaultExpressionV1::decode).map(Self::Element),
            2 => {
                decode_one(decoder, fields, DecodedDefaultExpressionV1::decode).map(Self::CopyArray)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

impl WireEncode for DecodedDefaultIntegerArgumentsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Unary(operand) => encode_one(encoder, 1, operand.as_ref()),
            Self::Binary { lhs, rhs } => encode_two(encoder, 2, lhs.as_ref(), rhs.as_ref()),
        }
    }
}

impl WireDecode for DecodedDefaultIntegerArgumentsV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => decode_boxed_expression(decoder, fields).map(Self::Unary),
            2 => {
                expect_sum_length(decoder, fields, 3)?;
                Ok(Self::Binary {
                    lhs: decode_boxed_expression_field(decoder, 1)?,
                    rhs: decode_boxed_expression_field(decoder, 2)?,
                })
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

fn decode_method_call(
    decoder: &mut Decoder<'_>,
    fields: u64,
    direct_super: bool,
) -> Result<DecodedDefaultExpressionKindV1, WireError> {
    expect_sum_length(decoder, fields, 4)?;
    let receiver = decode_boxed_expression_field(decoder, 1)?;
    let callee = decoder.field(2, DecodedDefaultMethodCalleeV1::decode)?;
    let arguments = decoder.field(3, decode_expression_array)?;
    if direct_super {
        Ok(DecodedDefaultExpressionKindV1::DirectSuperMethodCall {
            receiver,
            callee,
            arguments,
        })
    } else {
        Ok(DecodedDefaultExpressionKindV1::MethodCall {
            receiver,
            callee,
            arguments,
        })
    }
}

fn decode_expression_sequence(
    decoder: &mut Decoder<'_>,
    fields: u64,
) -> Result<Vec<DecodedDefaultExpressionV1>, WireError> {
    decode_one(decoder, fields, decode_expression_array)
}

fn decode_expression_array(
    decoder: &mut Decoder<'_>,
) -> Result<Vec<DecodedDefaultExpressionV1>, WireError> {
    decoder.decode_array(|decoder, _| DecodedDefaultExpressionV1::decode(decoder))
}

fn decode_boxed_expression(
    decoder: &mut Decoder<'_>,
    fields: u64,
) -> Result<Box<DecodedDefaultExpressionV1>, WireError> {
    expect_sum_length(decoder, fields, 2)?;
    decode_boxed_expression_field(decoder, 1)
}

fn decode_boxed_expression_field(
    decoder: &mut Decoder<'_>,
    field: u32,
) -> Result<Box<DecodedDefaultExpressionV1>, WireError> {
    decoder
        .field(field, DecodedDefaultExpressionV1::decode)
        .map(Box::new)
}

fn decode_one<T>(
    decoder: &mut Decoder<'_>,
    fields: u64,
    decode: impl FnOnce(&mut Decoder<'_>) -> Result<T, WireError>,
) -> Result<T, WireError> {
    expect_sum_length(decoder, fields, 2)?;
    decoder.field(1, decode)
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

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

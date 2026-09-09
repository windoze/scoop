//! Export-side source-call protocol for defaults, named arguments and varargs.

use super::*;

/// A canonical source point used by persistent locations and span endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePointRecord {
    byte_offset: u64,
    line: u64,
    column: u64,
}

impl SourcePointRecord {
    pub const fn byte_offset(&self) -> u64 {
        self.byte_offset
    }

    pub const fn line(&self) -> u64 {
        self.line
    }

    pub const fn column(&self) -> u64 {
        self.column
    }
}

impl scoop_wire::WireEncode for SourcePointRecord {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        encoder.unsigned(self.byte_offset)?;
        encoder.field(2)?;
        encoder.unsigned(self.line)?;
        encoder.field(3)?;
        encoder.unsigned(self.column)
    }
}

/// Stable source metadata. Full source text and host display locators are
/// deliberately absent and remain compile-session sidecar data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRecord {
    identity: scoop_identity::SourceIdentity,
    byte_length: u64,
    content_digest: scoop_identity::SourceContentDigest,
    line_starts: Vec<u64>,
    points: Vec<SourcePointRecord>,
}

impl SourceRecord {
    /// Build a record from trusted producer source text and every byte offset
    /// referenced by required metadata. Offsets are canonicalized into strict
    /// order and duplicate uses share one point.
    pub fn from_utf8(
        identity: scoop_identity::SourceIdentity,
        source: &str,
        point_offsets: impl IntoIterator<Item = u64>,
    ) -> Result<Self, SourceRecordError> {
        let byte_length =
            u64::try_from(source.len()).map_err(|_| SourceRecordError::LengthOverflow)?;

        let mut line_starts = Vec::new();
        line_starts
            .try_reserve_exact(
                source
                    .as_bytes()
                    .iter()
                    .filter(|byte| **byte == b'\n')
                    .count()
                    .checked_add(1)
                    .ok_or(SourceRecordError::LengthOverflow)?,
            )
            .map_err(|_| SourceRecordError::Allocation)?;
        line_starts.push(0);
        for (index, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                let start = index
                    .checked_add(1)
                    .and_then(|offset| u64::try_from(offset).ok())
                    .ok_or(SourceRecordError::LengthOverflow)?;
                line_starts.push(start);
            }
        }

        let mut canonical_offsets = Vec::new();
        for byte_offset in point_offsets {
            canonical_offsets
                .try_reserve(1)
                .map_err(|_| SourceRecordError::Allocation)?;
            canonical_offsets.push(byte_offset);
        }
        canonical_offsets.sort_unstable();
        canonical_offsets.dedup();
        let mut points = Vec::new();
        points
            .try_reserve_exact(canonical_offsets.len())
            .map_err(|_| SourceRecordError::Allocation)?;
        for byte_offset in canonical_offsets {
            if byte_offset > byte_length {
                return Err(SourceRecordError::PointPastEnd {
                    byte_offset,
                    byte_length,
                });
            }
            let offset =
                usize::try_from(byte_offset).map_err(|_| SourceRecordError::LengthOverflow)?;
            if !source.is_char_boundary(offset) {
                return Err(SourceRecordError::PointNotUtf8Boundary { byte_offset });
            }
            let line_index = line_starts.partition_point(|start| *start <= byte_offset) - 1;
            let line_start = usize::try_from(line_starts[line_index])
                .map_err(|_| SourceRecordError::LengthOverflow)?;
            let line = u64::try_from(line_index)
                .ok()
                .and_then(|line| line.checked_add(1))
                .ok_or(SourceRecordError::LengthOverflow)?;
            let column = u64::try_from(source[line_start..offset].chars().count())
                .ok()
                .and_then(|column| column.checked_add(1))
                .ok_or(SourceRecordError::LengthOverflow)?;
            points.push(SourcePointRecord {
                byte_offset,
                line,
                column,
            });
        }

        Ok(Self {
            identity,
            byte_length,
            content_digest: scoop_identity::SourceContentDigest::from_utf8(source),
            line_starts,
            points,
        })
    }

    pub const fn identity(&self) -> &scoop_identity::SourceIdentity {
        &self.identity
    }

    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }

    pub const fn content_digest(&self) -> scoop_identity::SourceContentDigest {
        self.content_digest
    }

    pub fn line_starts(&self) -> &[u64] {
        &self.line_starts
    }

    pub fn points(&self) -> &[SourcePointRecord] {
        &self.points
    }

    pub fn point(&self, byte_offset: u64) -> Option<SourcePointRecord> {
        self.points
            .binary_search_by_key(&byte_offset, SourcePointRecord::byte_offset)
            .ok()
            .map(|index| self.points[index])
    }
}

impl scoop_wire::WireEncode for SourceRecord {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(5)?;
        encoder.field(1)?;
        scoop_wire::WireEncode::encode(&self.identity, encoder)?;
        encoder.field(2)?;
        encoder.unsigned(self.byte_length)?;
        encoder.field(3)?;
        scoop_wire::WireEncode::encode(&self.content_digest, encoder)?;
        encoder.field(4)?;
        encoder.array(self.line_starts.len() as u64)?;
        for line_start in &self.line_starts {
            encoder.unsigned(*line_start)?;
        }
        encoder.field(5)?;
        encoder.array(self.points.len() as u64)?;
        for point in &self.points {
            scoop_wire::WireEncode::encode(point, encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRecordError {
    LengthOverflow,
    Allocation,
    PointPastEnd { byte_offset: u64, byte_length: u64 },
    PointNotUtf8Boundary { byte_offset: u64 },
}

impl std::fmt::Display for SourceRecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LengthOverflow => formatter.write_str("source byte length does not fit u64"),
            Self::Allocation => formatter.write_str("failed to allocate source metadata"),
            Self::PointPastEnd {
                byte_offset,
                byte_length,
            } => write!(
                formatter,
                "source point offset {byte_offset} exceeds source byte length {byte_length}"
            ),
            Self::PointNotUtf8Boundary { byte_offset } => write!(
                formatter,
                "source point offset {byte_offset} is not a UTF-8 scalar boundary"
            ),
        }
    }
}

impl std::error::Error for SourceRecordError {}

#[cfg(test)]
mod source_record_tests {
    use super::*;

    #[test]
    fn producer_builds_canonical_utf8_crlf_points_and_wire() {
        let source = "a\r\n雪\n";
        let record = SourceRecord::from_utf8(
            scoop_identity::SourceIdentity::single_file(),
            source,
            [7, 2, 3, 6, 0, 3],
        )
        .unwrap();

        assert_eq!(record.byte_length(), 7);
        assert_eq!(
            record.content_digest().to_string(),
            "da657a9242d365403e34959133543789fad5d1d7ba0a1a21011fa09c6cbc299d"
        );
        assert_eq!(record.line_starts(), &[0, 3, 7]);
        assert_eq!(
            record.points(),
            &[
                SourcePointRecord {
                    byte_offset: 0,
                    line: 1,
                    column: 1,
                },
                SourcePointRecord {
                    byte_offset: 2,
                    line: 1,
                    column: 3,
                },
                SourcePointRecord {
                    byte_offset: 3,
                    line: 2,
                    column: 1,
                },
                SourcePointRecord {
                    byte_offset: 6,
                    line: 2,
                    column: 2,
                },
                SourcePointRecord {
                    byte_offset: 7,
                    line: 3,
                    column: 1,
                },
            ]
        );
        assert_eq!(record.point(6).unwrap().column(), 2);
        assert!(record.point(1).is_none());

        assert_eq!(
            scoop_wire::encode(&record).unwrap(),
            [
                b"\xa5\x01\xa2\x01\x58\x20".as_slice(),
                scoop_identity::ConeIdentity::SINGLE_FILE.as_array(),
                b"\x02\x6amain.scoop\x02\x07\x03\x58\x20".as_slice(),
                record.content_digest().as_array(),
                b"\x04\x83\x00\x03\x07\x05\x85\
                  \xa3\x01\x00\x02\x01\x03\x01\
                  \xa3\x01\x02\x02\x01\x03\x03\
                  \xa3\x01\x03\x02\x02\x03\x01\
                  \xa3\x01\x06\x02\x02\x03\x02\
                  \xa3\x01\x07\x02\x03\x03\x01"
                    .as_slice(),
            ]
            .concat()
        );
    }

    #[test]
    fn producer_rejects_invalid_source_point_offsets() {
        let identity = scoop_identity::SourceIdentity::single_file();
        assert_eq!(
            SourceRecord::from_utf8(identity.clone(), "雪", [1]),
            Err(SourceRecordError::PointNotUtf8Boundary { byte_offset: 1 })
        );
        assert_eq!(
            SourceRecord::from_utf8(identity, "雪", [4]),
            Err(SourceRecordError::PointPastEnd {
                byte_offset: 4,
                byte_length: 3,
            })
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceFileMetadata {
    pub provider: IntrinsicProviderId,
    pub identity: scoop_identity::SourceIdentity,
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceContext {
    pub function_name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DefinitionOrigin {
    pub provider: IntrinsicProviderId,
    pub file: u32,
    pub span: Span,
    pub context: SourceContextId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationOrigin {
    pub provider: IntrinsicProviderId,
    pub file: u32,
    pub span: Span,
    pub context: SourceContextId,
}

impl From<DefinitionOrigin> for EvaluationOrigin {
    fn from(origin: DefinitionOrigin) -> Self {
        Self {
            provider: origin.provider,
            file: origin.file,
            span: origin.span,
            context: origin.context,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConcreteExpressionOrigin {
    pub definition: DefinitionOrigin,
    pub evaluation: EvaluationOrigin,
}

/// Export HIR contains both declaration-bound expression bodies and default
/// instances embedded in ordinary bodies. A template node has definition
/// provenance only; the concrete product closes the first branch by using its
/// own definition as the evaluation source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpressionOrigin {
    Definition(DefinitionOrigin),
    Instantiated(ConcreteExpressionOrigin),
}

impl ExpressionOrigin {
    pub const fn definition(self) -> DefinitionOrigin {
        match self {
            Self::Definition(definition)
            | Self::Instantiated(ConcreteExpressionOrigin { definition, .. }) => definition,
        }
    }

    pub fn concrete(self) -> ConcreteExpressionOrigin {
        match self {
            Self::Definition(definition) => ConcreteExpressionOrigin {
                definition,
                evaluation: definition.into(),
            },
            Self::Instantiated(origin) => origin,
        }
    }

    pub const fn instantiate(self, evaluation: EvaluationOrigin) -> Self {
        Self::Instantiated(ConcreteExpressionOrigin {
            definition: self.definition(),
            evaluation,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ExportValueParameter {
    pub name: String,
    pub calling: ExportParameterCalling,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportParameterCalling {
    Required {
        value_type: TypeId,
    },
    Default {
        value_type: TypeId,
        source: ExportDefaultSourceId,
    },
    Vararg {
        parameter_type: ExportVarargParameterTypeId,
        omission: ExportVarargOmission,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportVarargOmission {
    EmptyArray,
    Default(ExportDefaultSourceId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportVarargParameterType {
    pub element_type: TypeId,
    pub array_type: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportParameterOwner {
    Function(FunctionId),
    StructConstructor(StructConstructorId),
    ClassConstructor(ClassConstructorId),
    VariantConstructor(EnumVariantRef),
}

#[derive(Debug, Clone)]
pub struct ExportParameterInterface {
    pub owner: ExportParameterOwner,
    pub parameters: Vec<ExportValueParameter>,
}

/// A default is a declaration-bound typed body. Parameter references are
/// locals in this template arena and are related to declaration positions by
/// `value_parameters`; callers never resolve their source names again.
#[derive(Debug, Clone)]
pub struct ExportDefaultExpr {
    pub locals: Arena<Local>,
    pub statements: Vec<Statement>,
    pub value: Expr,
    pub result_type: TypeId,
    /// Whether this declaration-bound region may contain suspend calls.
    /// The reader boundary checks this against every source-parameter owner
    /// that references the template.
    pub allows_suspend: bool,
    /// The exact declaration identities referenced by `Type::Param` nodes in
    /// the template. An inherited source relates these to its own static view
    /// through `ExportDefaultSource::type_arguments`.
    pub type_parameters: Vec<TypeParamId>,
    pub receiver: Option<ExportDefaultReceiver>,
    pub value_parameters: Vec<ExportDefaultValueParameter>,
    /// Direct declaration-bound dependencies of the typed template. Each
    /// category has its own identity domain and every entry carries the
    /// access-domain proof produced before this export entity is committed.
    pub references: ExportDefaultReferences,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Default)]
pub struct ExportDefaultReferences {
    pub callables: Vec<ExportDefaultCallableRef>,
    pub constructors: Vec<ExportDefaultConstructorRef>,
    pub types: Vec<ExportDefaultTypeRef>,
    pub globals: Vec<ExportDefaultGlobalRef>,
    pub singleton_values: Vec<ExportDefaultSingletonValueRef>,
    pub fields: Vec<ExportDefaultFieldRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultAccessWitness {
    pub owner: ExportParameterOwner,
    pub call_domain: CallDomain,
    pub target_domain: AccessDomain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallDomain {
    pub direct: EffectiveLookupDomain,
    pub slot: Option<SlotContractDomain>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultCallableRef {
    pub target: ExportDefaultCallableTarget,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDefaultCallableTarget {
    Callable(Callable),
    Bound(BoundCallableRefId),
    DerivedEquality(DerivedEqualityApplicationId),
    LocalFunction(LocalFunctionId),
    Lambda(LambdaId),
    AnonymousFunction(AnonymousFunctionId),
    CallableReference(CallableReferenceId),
    FunctionAddress(FunctionId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultConstructorRef {
    pub target: ExportDefaultConstructorTarget,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportDefaultConstructorTarget {
    Struct(StructConstructorApplicationId),
    Class(ClassConstructorApplicationId),
    Variant(AppliedEnumVariantRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultTypeRef {
    pub target: TypeId,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultGlobalRef {
    pub target: GlobalId,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultSingletonValueRef {
    pub target: SingletonValueId,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultFieldRef {
    pub target: FieldRef,
    pub witness: ExportDefaultAccessWitness,
    pub origin: DefinitionOrigin,
}

/// A typed inheritance/application edge for one default source. The argument
/// at each position corresponds to the template parameter at the same
/// position and is expressed in the consuming declaration's type scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportDefaultSource {
    pub expression: ExportDefaultExprId,
    pub type_arguments: Vec<TypeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDefaultReceiver {
    pub local: LocalId,
    pub ty: TypeId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportDefaultValueParameter {
    pub position: u32,
    pub local: LocalId,
}

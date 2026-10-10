use super::*;

impl DefaultWhenV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultWhenV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        let subject = self
            .subject
            .as_ref()
            .map(|subject| {
                subject.index_locals(resolver).map_err(|error| {
                    DefaultControlFlowIndexError::Expression {
                        context: "when subject",
                        error,
                    }
                })
            })
            .transpose()?;
        let mut arms = Vec::with_capacity(self.arms.len());
        for (index, arm) in self.arms.iter().enumerate() {
            arms.push(arm.index_locals(resolver, index)?);
        }
        Ok(IndexedDefaultWhenV1 {
            subject,
            arms,
            fallback: self.fallback.index_locals(resolver)?,
        })
    }
}

impl DefaultWhenArmV1 {
    fn index_locals<I>(
        &self,
        resolver: &mut I,
        arm_index: usize,
    ) -> Result<IndexedDefaultWhenArmV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        Ok(IndexedDefaultWhenArmV1 {
            condition: match &self.condition {
                DefaultWhenConditionV1::Case(pattern) => {
                    IndexedDefaultWhenConditionV1::Case(pattern.index_locals(resolver).map_err(
                        |error| DefaultControlFlowIndexError::Pattern { arm_index, error },
                    )?)
                }
                DefaultWhenConditionV1::Predicate(predicate) => {
                    IndexedDefaultWhenConditionV1::Predicate(predicate.index_locals(resolver)?)
                }
                DefaultWhenConditionV1::Always => IndexedDefaultWhenConditionV1::Always,
            },
            guard: self.guard.index_locals(resolver)?,
            body: index_statements(&self.body, resolver, "when arm body")?,
            definition_origin: &self.definition_origin,
        })
    }
}

impl OptionalDefaultWhenGuardV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedOptionalDefaultWhenGuardV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        match self {
            Self::Absent => Ok(IndexedOptionalDefaultWhenGuardV1::Absent),
            Self::Present(guard) => guard
                .index_locals(resolver)
                .map(IndexedOptionalDefaultWhenGuardV1::Present),
        }
    }
}

impl DefaultWhenGuardV1 {
    fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultWhenGuardV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        Ok(IndexedDefaultWhenGuardV1 {
            setup: index_statements(&self.setup, resolver, "when guard setup")?,
            condition: self.condition.index_locals(resolver).map_err(|error| {
                DefaultControlFlowIndexError::Expression {
                    context: "when guard condition",
                    error,
                }
            })?,
        })
    }
}

impl DefaultWhenFallbackV1 {
    pub fn index_locals<I>(
        &self,
        resolver: &mut I,
    ) -> Result<IndexedDefaultWhenFallbackV1<'_>, DefaultControlFlowIndexError<I::Error>>
    where
        I: TemplateLocalIndexResolver,
    {
        match self.view() {
            DefaultWhenFallbackViewV1::Fallthrough => Ok(IndexedDefaultWhenFallbackV1::Fallthrough),
            DefaultWhenFallbackViewV1::Else(statements) => {
                index_statements(statements, resolver, "when else")
                    .map(IndexedDefaultWhenFallbackV1::Else)
            }
            DefaultWhenFallbackViewV1::IrrefutableArm { subject_type } => {
                Ok(IndexedDefaultWhenFallbackV1::IrrefutableArm { subject_type })
            }
            DefaultWhenFallbackViewV1::PatternMatrix { subject_type } => {
                Ok(IndexedDefaultWhenFallbackV1::PatternMatrix { subject_type })
            }
            DefaultWhenFallbackViewV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => Ok(IndexedDefaultWhenFallbackV1::EnumPatternMatrix {
                subject_type,
                owner_type,
            }),
        }
    }
}

impl WireEncode for IndexedDefaultWhenV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(3)?;
        encoder.field(1)?;
        match &self.subject {
            Some(subject) => encode_one(encoder, 2, subject)?,
            None => encode_empty(encoder, 1)?,
        }
        encoder.field(2)?;
        encode_sequence(encoder, &self.arms)?;
        encoder.field(3)?;
        self.fallback.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultWhenArmV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        match &self.condition {
            IndexedDefaultWhenConditionV1::Case(pattern) => encode_one(encoder, 1, pattern)?,
            IndexedDefaultWhenConditionV1::Predicate(predicate) => {
                encode_one(encoder, 2, predicate)?
            }
            IndexedDefaultWhenConditionV1::Always => encode_empty(encoder, 3)?,
        }
        encoder.field(2)?;
        self.guard.encode(encoder)?;
        encoder.field(3)?;
        encode_sequence(encoder, &self.body)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)
    }
}

impl WireEncode for IndexedOptionalDefaultWhenGuardV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Absent => encode_empty(encoder, 1),
            Self::Present(guard) => encode_one(encoder, 2, guard),
        }
    }
}

impl WireEncode for IndexedDefaultWhenGuardV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        encode_sequence(encoder, &self.setup)?;
        encoder.field(2)?;
        self.condition.encode(encoder)
    }
}

impl WireEncode for IndexedDefaultWhenFallbackV1<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Fallthrough => encode_empty(encoder, 5),
            Self::Else(statements) => encode_one(encoder, 1, &WireSequence(statements)),
            Self::IrrefutableArm { subject_type } => encode_one(encoder, 2, *subject_type),
            Self::PatternMatrix { subject_type } => encode_one(encoder, 3, *subject_type),
            Self::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => encode_two(encoder, 4, *subject_type, *owner_type),
        }
    }
}

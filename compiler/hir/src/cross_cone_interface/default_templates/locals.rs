use scoop_identity::{
    ConeIdentity, DecodedSignatureTypeKey, LocalValueSelector, PersistentIdResolver,
    PersistentKeyResolver, PersistentSourceContextId, SignatureTypeKey, SourceContextKey,
    SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use crate::{
    CanonicalBooleanV1, DecodedExportDefinitionSourceV1, ExportDefinitionSourceV1,
    SignatureTypeReferenceResolver,
};

mod errors;

pub use errors::{
    TemplateLocalLookupError, TemplateLocalRecordBuildError, TemplateLocalRecordResolutionError,
    TemplateLocalTableBuildError, TemplateLocalTableValidationError,
};

/// Resolves one canonical wire-local table index to its semantic selector.
pub trait TemplateLocalSelectorResolver {
    type Error;

    fn resolve_template_local_selector(
        &mut self,
        index: u32,
    ) -> Result<LocalValueSelector, Self::Error>;
}

/// Maps one semantic local selector to its canonical wire table index.
pub trait TemplateLocalIndexResolver {
    type Error;

    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateLocalDefinitionV1 {
    Source(ExportDefinitionSourceV1),
    Synthetic,
}

impl WireEncode for TemplateLocalDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Source(origin) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                origin.encode(encoder)
            }
            Self::Synthetic => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(2)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodedTemplateLocalDefinitionV1 {
    Source(DecodedExportDefinitionSourceV1),
    Synthetic,
}

impl DecodedTemplateLocalDefinitionV1 {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<TemplateLocalDefinitionV1, SourceOriginResolutionError<E>>
    where
        R: TemplateLocalReferenceResolver<E>,
    {
        match self {
            Self::Source(origin) => origin
                .resolve(resolver)
                .map(TemplateLocalDefinitionV1::Source),
            Self::Synthetic => Ok(TemplateLocalDefinitionV1::Synthetic),
        }
    }
}

impl WireEncode for DecodedTemplateLocalDefinitionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Source(origin) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                origin.encode(encoder)
            }
            Self::Synthetic => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(2)
            }
        }
    }
}

impl WireDecode for DecodedTemplateLocalDefinitionV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        let tag = decoder.field(0, Decoder::unsigned)?;
        match tag {
            1 => {
                expect_sum_length(decoder, fields, 2)?;
                decoder
                    .field(1, DecodedExportDefinitionSourceV1::decode)
                    .map(Self::Source)
            }
            2 => {
                expect_sum_length(decoder, fields, 1)?;
                Ok(Self::Synthetic)
            }
            tag => Err(wire_error(decoder, WireErrorKind::UnknownTag { tag })),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateLocalRecordV1 {
    selector: LocalValueSelector,
    value_type: SignatureTypeKey,
    mutable: CanonicalBooleanV1,
    definition: TemplateLocalDefinitionV1,
}

impl TemplateLocalRecordV1 {
    pub fn try_new(
        selector: LocalValueSelector,
        value_type: SignatureTypeKey,
        mutable: CanonicalBooleanV1,
        definition: TemplateLocalDefinitionV1,
    ) -> Result<Self, TemplateLocalRecordBuildError> {
        validate_definition_shape(&selector, &definition)?;
        Ok(Self {
            selector,
            value_type,
            mutable,
            definition,
        })
    }

    pub const fn selector(&self) -> &LocalValueSelector {
        &self.selector
    }

    pub const fn value_type(&self) -> &SignatureTypeKey {
        &self.value_type
    }

    pub const fn mutable(&self) -> CanonicalBooleanV1 {
        self.mutable
    }

    pub const fn definition(&self) -> &TemplateLocalDefinitionV1 {
        &self.definition
    }
}

impl WireEncode for TemplateLocalRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.selector.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.mutable.encode(encoder)?;
        encoder.field(4)?;
        self.definition.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTemplateLocalRecordV1 {
    selector: LocalValueSelector,
    value_type: DecodedSignatureTypeKey,
    mutable: CanonicalBooleanV1,
    definition: DecodedTemplateLocalDefinitionV1,
}

impl DecodedTemplateLocalRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<TemplateLocalRecordV1, TemplateLocalRecordResolutionError<E>>
    where
        R: TemplateLocalReferenceResolver<E>,
    {
        let value_type = self
            .value_type
            .resolve(resolver)
            .map_err(TemplateLocalRecordResolutionError::Type)?;
        let definition = self
            .definition
            .resolve(resolver)
            .map_err(TemplateLocalRecordResolutionError::Definition)?;
        TemplateLocalRecordV1::try_new(self.selector, value_type, self.mutable, definition)
            .map_err(TemplateLocalRecordResolutionError::Shape)
    }
}

impl WireEncode for DecodedTemplateLocalRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.selector.encode(encoder)?;
        encoder.field(2)?;
        self.value_type.encode(encoder)?;
        encoder.field(3)?;
        self.mutable.encode(encoder)?;
        encoder.field(4)?;
        self.definition.encode(encoder)
    }
}

impl WireDecode for DecodedTemplateLocalRecordV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            selector: decoder.field(1, LocalValueSelector::decode)?,
            value_type: decoder.field(2, DecodedSignatureTypeKey::decode)?,
            mutable: decoder.field(3, CanonicalBooleanV1::decode)?,
            definition: decoder.field(4, DecodedTemplateLocalDefinitionV1::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalTemplateLocalTableV1 {
    records: Vec<TemplateLocalRecordV1>,
    len: u32,
}

impl CanonicalTemplateLocalTableV1 {
    pub fn try_new(
        mut records: Vec<TemplateLocalRecordV1>,
    ) -> Result<Self, TemplateLocalTableBuildError> {
        let len =
            u32::try_from(records.len()).map_err(|_| TemplateLocalTableBuildError::TooMany)?;
        records.sort_unstable_by(|left, right| left.selector.cmp(&right.selector));
        if let Some(selector) = duplicate_selector(&records) {
            return Err(TemplateLocalTableBuildError::Duplicate(selector));
        }
        Ok(Self { records, len })
    }

    pub fn records(&self) -> &[TemplateLocalRecordV1] {
        &self.records
    }

    pub const fn len_u32(&self) -> u32 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn get(&self, selector: &LocalValueSelector) -> Option<&TemplateLocalRecordV1> {
        self.records
            .binary_search_by(|record| record.selector.cmp(selector))
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn index_of(&self, selector: &LocalValueSelector) -> Option<u32> {
        self.records
            .binary_search_by(|record| record.selector.cmp(selector))
            .ok()
            .and_then(|index| u32::try_from(index).ok())
    }
}

impl WireEncode for CanonicalTemplateLocalTableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl TemplateLocalSelectorResolver for CanonicalTemplateLocalTableV1 {
    type Error = TemplateLocalLookupError;

    fn resolve_template_local_selector(
        &mut self,
        index: u32,
    ) -> Result<LocalValueSelector, Self::Error> {
        usize::try_from(index)
            .ok()
            .and_then(|index| self.records.get(index))
            .map(|record| record.selector.clone())
            .ok_or(TemplateLocalLookupError::IndexOutOfRange {
                index,
                len: self.len,
            })
    }
}

impl TemplateLocalIndexResolver for CanonicalTemplateLocalTableV1 {
    type Error = TemplateLocalLookupError;

    fn resolve_template_local_index(
        &mut self,
        selector: &LocalValueSelector,
    ) -> Result<u32, Self::Error> {
        self.index_of(selector)
            .ok_or_else(|| TemplateLocalLookupError::MissingSelector(selector.clone()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalTemplateLocalTableV1 {
    records: Vec<DecodedTemplateLocalRecordV1>,
}

impl DecodedCanonicalTemplateLocalTableV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalTemplateLocalTableV1, TemplateLocalTableValidationError<E>>
    where
        R: TemplateLocalReferenceResolver<E>,
    {
        let len = u32::try_from(self.records.len())
            .map_err(|_| TemplateLocalTableValidationError::TooMany)?;
        let mut records: Vec<TemplateLocalRecordV1> = Vec::with_capacity(self.records.len());
        for (index, record) in self.records.into_iter().enumerate() {
            let record = record
                .resolve(resolver)
                .map_err(|error| TemplateLocalTableValidationError::Record { index, error })?;
            if let Some(previous) = records.last() {
                match previous.selector.cmp(&record.selector) {
                    std::cmp::Ordering::Equal => {
                        return Err(TemplateLocalTableValidationError::Duplicate {
                            index,
                            selector: record.selector,
                        });
                    }
                    std::cmp::Ordering::Greater => {
                        return Err(TemplateLocalTableValidationError::NonCanonicalOrder { index });
                    }
                    std::cmp::Ordering::Less => {}
                }
            }
            records.push(record);
        }
        Ok(CanonicalTemplateLocalTableV1 { records, len })
    }
}

impl WireEncode for DecodedCanonicalTemplateLocalTableV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedCanonicalTemplateLocalTableV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder
            .decode_array(|decoder, _| DecodedTemplateLocalRecordV1::decode(decoder))
            .map(|records| Self { records })
    }
}

pub trait TemplateLocalReferenceResolver<E>:
    SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl<R, E> TemplateLocalReferenceResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

fn validate_definition_shape(
    selector: &LocalValueSelector,
    definition: &TemplateLocalDefinitionV1,
) -> Result<(), TemplateLocalRecordBuildError> {
    match (selector, definition) {
        (LocalValueSelector::SuspensionResult { .. }, _) => Err(
            TemplateLocalRecordBuildError::UnsupportedSelector(selector.clone()),
        ),
        (LocalValueSelector::Synthetic { .. }, TemplateLocalDefinitionV1::Source(_)) => Err(
            TemplateLocalRecordBuildError::SyntheticDefinitionRequired(selector.clone()),
        ),
        (
            LocalValueSelector::This
            | LocalValueSelector::Parameter { .. }
            | LocalValueSelector::LocalDeclaration { .. }
            | LocalValueSelector::BoundReceiver { .. },
            TemplateLocalDefinitionV1::Synthetic,
        ) => Err(TemplateLocalRecordBuildError::SourceDefinitionRequired(
            selector.clone(),
        )),
        (
            LocalValueSelector::This
            | LocalValueSelector::Parameter { .. }
            | LocalValueSelector::LocalDeclaration { .. }
            | LocalValueSelector::BoundReceiver { .. },
            TemplateLocalDefinitionV1::Source(_),
        )
        | (LocalValueSelector::Synthetic { .. }, TemplateLocalDefinitionV1::Synthetic) => Ok(()),
    }
}

fn duplicate_selector(records: &[TemplateLocalRecordV1]) -> Option<LocalValueSelector> {
    records
        .windows(2)
        .find(|pair| pair[0].selector == pair[1].selector)
        .map(|pair| pair[0].selector.clone())
}

fn expect_sum_length(
    decoder: &Decoder<'_, '_>,
    actual: u64,
    expected: u64,
) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(wire_error(
            decoder,
            WireErrorKind::InvalidLength { expected, actual },
        ))
    }
}

fn wire_error(decoder: &Decoder<'_, '_>, kind: WireErrorKind) -> WireError {
    WireError::new(kind, decoder.path().clone(), Some(decoder.position()))
}

#[cfg(test)]
mod tests;

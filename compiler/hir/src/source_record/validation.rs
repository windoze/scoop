//! Wire decoding and structural validation for source metadata.

use super::{SourcePointRecord, SourceRecord, SourceRecordError};
use scoop_identity::{ConeIdentity, PersistentIdResolver, SourceIdentityResolutionError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedSourcePointRecord {
    pub(super) byte_offset: u64,
    pub(super) line: u64,
    pub(super) column: u64,
}

impl scoop_wire::WireEncode for DecodedSourcePointRecord {
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

impl scoop_wire::WireDecode for DecodedSourcePointRecord {
    fn decode(decoder: &mut scoop_wire::Decoder<'_, '_>) -> Result<Self, scoop_wire::WireError> {
        decoder.expect_map(3)?;
        let byte_offset = decoder.field(1, scoop_wire::Decoder::unsigned)?;
        let line = decoder.field(2, scoop_wire::Decoder::unsigned)?;
        let column = decoder.field(3, scoop_wire::Decoder::unsigned)?;
        Ok(Self {
            byte_offset,
            line,
            column,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSourceRecord {
    pub(super) identity: scoop_identity::DecodedSourceIdentity,
    pub(super) byte_length: u64,
    pub(super) content_digest: scoop_identity::DecodedSourceContentDigest,
    pub(super) line_starts: Vec<u64>,
    pub(super) points: Vec<DecodedSourcePointRecord>,
}

impl DecodedSourceRecord {
    /// Validate metadata that does not require source bytes. Supplying a source
    /// attachment additionally recomputes the digest, line starts, UTF-8
    /// boundaries, and Unicode-scalar columns.
    pub fn validate(
        self,
        coordinate: &scoop_identity::ConeCoordinate,
        source_attachment: Option<&str>,
    ) -> Result<SourceRecord, SourceRecordValidationError> {
        let Self {
            identity,
            byte_length,
            content_digest,
            line_starts,
            points,
        } = self;
        let identity = identity
            .validate(coordinate)
            .map_err(SourceRecordValidationError::Identity)?;
        validate_resolved_record(
            identity,
            byte_length,
            content_digest,
            line_starts,
            points,
            source_attachment,
        )
    }

    /// Resolves the source Cone through an already validated dependency
    /// identity graph, then validates the canonical source metadata.
    pub fn resolve<R>(
        self,
        resolver: &mut R,
        source_attachment: Option<&str>,
    ) -> Result<SourceRecord, SourceRecordResolutionError<R::Error>>
    where
        R: PersistentIdResolver<ConeIdentity>,
    {
        let Self {
            identity,
            byte_length,
            content_digest,
            line_starts,
            points,
        } = self;
        let identity = identity
            .resolve(resolver)
            .map_err(SourceRecordResolutionError::Identity)?;
        validate_resolved_record(
            identity,
            byte_length,
            content_digest,
            line_starts,
            points,
            source_attachment,
        )
        .map_err(SourceRecordResolutionError::Metadata)
    }
}

fn validate_resolved_record(
    identity: scoop_identity::SourceIdentity,
    byte_length: u64,
    content_digest: scoop_identity::DecodedSourceContentDigest,
    line_starts: Vec<u64>,
    decoded_points: Vec<DecodedSourcePointRecord>,
    source_attachment: Option<&str>,
) -> Result<SourceRecord, SourceRecordValidationError> {
    let content_digest = content_digest
        .validate()
        .map_err(SourceRecordValidationError::ContentDigest)?;
    validate_line_starts(&line_starts, byte_length)?;
    let points = validate_points(&decoded_points, &line_starts, byte_length)?;
    let record = SourceRecord {
        identity,
        byte_length,
        content_digest,
        line_starts,
        points,
    };

    let Some(source) = source_attachment else {
        return Ok(record);
    };
    let expected = SourceRecord::from_utf8(
        record.identity.clone(),
        source,
        record.points.iter().map(SourcePointRecord::byte_offset),
    )
    .map_err(SourceRecordValidationError::SourceAttachment)?;
    if record.byte_length != expected.byte_length {
        return Err(SourceRecordValidationError::SourceLengthMismatch {
            declared: record.byte_length,
            actual: expected.byte_length,
        });
    }
    if record.content_digest != expected.content_digest {
        return Err(SourceRecordValidationError::SourceDigestMismatch);
    }
    if record.line_starts != expected.line_starts {
        return Err(SourceRecordValidationError::SourceLineStartsMismatch);
    }
    if record.points != expected.points {
        return Err(SourceRecordValidationError::SourcePointsMismatch);
    }
    Ok(record)
}

impl scoop_wire::WireEncode for DecodedSourceRecord {
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

impl scoop_wire::WireDecode for DecodedSourceRecord {
    fn decode(decoder: &mut scoop_wire::Decoder<'_, '_>) -> Result<Self, scoop_wire::WireError> {
        decoder.expect_map(5)?;
        let identity = decoder.field(1, scoop_identity::DecodedSourceIdentity::decode)?;
        let byte_length = decoder.field(2, scoop_wire::Decoder::unsigned)?;
        let content_digest =
            decoder.field(3, scoop_identity::DecodedSourceContentDigest::decode)?;
        let line_starts = decoder.field(4, |decoder| {
            decoder.decode_array(|decoder, _| decoder.unsigned())
        })?;
        let points = decoder.field(5, |decoder| {
            decoder.decode_array(|decoder, _| DecodedSourcePointRecord::decode(decoder))
        })?;
        Ok(Self {
            identity,
            byte_length,
            content_digest,
            line_starts,
            points,
        })
    }
}

fn validate_line_starts(
    line_starts: &[u64],
    byte_length: u64,
) -> Result<(), SourceRecordValidationError> {
    if line_starts.first() != Some(&0) {
        return Err(SourceRecordValidationError::MissingInitialLineStart);
    }
    for (index, window) in line_starts.windows(2).enumerate() {
        if window[0] >= window[1] {
            return Err(SourceRecordValidationError::NonIncreasingLineStarts {
                first_index: index,
                second_index: index + 1,
            });
        }
    }
    if let Some(&byte_offset) = line_starts.last()
        && byte_offset > byte_length
    {
        return Err(SourceRecordValidationError::LineStartPastEnd {
            byte_offset,
            byte_length,
        });
    }
    Ok(())
}

fn validate_points(
    decoded: &[DecodedSourcePointRecord],
    line_starts: &[u64],
    byte_length: u64,
) -> Result<Vec<SourcePointRecord>, SourceRecordValidationError> {
    let mut points = Vec::new();
    points
        .try_reserve_exact(decoded.len())
        .map_err(|_| SourceRecordValidationError::Allocation)?;
    for (index, point) in decoded.iter().enumerate() {
        if index > 0 && decoded[index - 1].byte_offset >= point.byte_offset {
            return Err(SourceRecordValidationError::NonIncreasingPoints {
                first_index: index - 1,
                second_index: index,
            });
        }
        if point.byte_offset > byte_length {
            return Err(SourceRecordValidationError::PointPastEnd {
                index,
                byte_offset: point.byte_offset,
                byte_length,
            });
        }
        let line_index = point
            .line
            .checked_sub(1)
            .and_then(|line| usize::try_from(line).ok())
            .ok_or(SourceRecordValidationError::InvalidPointLine {
                index,
                line: point.line,
            })?;
        let line_start = line_starts.get(line_index).copied().ok_or(
            SourceRecordValidationError::InvalidPointLine {
                index,
                line: point.line,
            },
        )?;
        if point.column == 0 {
            return Err(SourceRecordValidationError::ZeroPointColumn { index });
        }
        let before_start = point.byte_offset < line_start;
        let after_end = line_starts
            .get(line_index + 1)
            .is_some_and(|next| point.byte_offset >= *next);
        if before_start || after_end {
            return Err(SourceRecordValidationError::PointOutsideDeclaredLine {
                index,
                byte_offset: point.byte_offset,
                line: point.line,
            });
        }
        points.push(SourcePointRecord {
            byte_offset: point.byte_offset,
            line: point.line,
            column: point.column,
        });
    }
    Ok(points)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRecordValidationError {
    Identity(scoop_identity::SourceIdentityDecodeError),
    ContentDigest(scoop_identity::SourceContentDigestError),
    MissingInitialLineStart,
    NonIncreasingLineStarts {
        first_index: usize,
        second_index: usize,
    },
    LineStartPastEnd {
        byte_offset: u64,
        byte_length: u64,
    },
    NonIncreasingPoints {
        first_index: usize,
        second_index: usize,
    },
    PointPastEnd {
        index: usize,
        byte_offset: u64,
        byte_length: u64,
    },
    InvalidPointLine {
        index: usize,
        line: u64,
    },
    ZeroPointColumn {
        index: usize,
    },
    PointOutsideDeclaredLine {
        index: usize,
        byte_offset: u64,
        line: u64,
    },
    Allocation,
    SourceAttachment(SourceRecordError),
    SourceLengthMismatch {
        declared: u64,
        actual: u64,
    },
    SourceDigestMismatch,
    SourceLineStartsMismatch,
    SourcePointsMismatch,
}

impl std::fmt::Display for SourceRecordValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::ContentDigest(error) => error.fmt(formatter),
            Self::MissingInitialLineStart => {
                formatter.write_str("source line starts must begin with byte offset 0")
            }
            Self::NonIncreasingLineStarts {
                first_index,
                second_index,
            } => write!(
                formatter,
                "source line starts {first_index} and {second_index} are not strictly increasing"
            ),
            Self::LineStartPastEnd {
                byte_offset,
                byte_length,
            } => write!(
                formatter,
                "source line start {byte_offset} exceeds byte length {byte_length}"
            ),
            Self::NonIncreasingPoints {
                first_index,
                second_index,
            } => write!(
                formatter,
                "source points {first_index} and {second_index} are not strictly increasing"
            ),
            Self::PointPastEnd {
                index,
                byte_offset,
                byte_length,
            } => write!(
                formatter,
                "source point {index} offset {byte_offset} exceeds byte length {byte_length}"
            ),
            Self::InvalidPointLine { index, line } => {
                write!(
                    formatter,
                    "source point {index} uses undeclared line {line}"
                )
            }
            Self::ZeroPointColumn { index } => {
                write!(formatter, "source point {index} column must be at least 1")
            }
            Self::PointOutsideDeclaredLine {
                index,
                byte_offset,
                line,
            } => write!(
                formatter,
                "source point {index} offset {byte_offset} is outside declared line {line}"
            ),
            Self::Allocation => formatter.write_str("failed to allocate validated source points"),
            Self::SourceAttachment(error) => error.fmt(formatter),
            Self::SourceLengthMismatch { declared, actual } => write!(
                formatter,
                "source attachment byte length {actual} does not match declared length {declared}"
            ),
            Self::SourceDigestMismatch => {
                formatter.write_str("source attachment digest does not match the source record")
            }
            Self::SourceLineStartsMismatch => {
                formatter.write_str("source attachment line starts do not match the source record")
            }
            Self::SourcePointsMismatch => formatter
                .write_str("source attachment point coordinates do not match the source record"),
        }
    }
}

impl std::error::Error for SourceRecordValidationError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceRecordResolutionError<E> {
    Identity(SourceIdentityResolutionError<E>),
    Metadata(SourceRecordValidationError),
}

impl<E: std::fmt::Display> std::fmt::Display for SourceRecordResolutionError<E> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identity(error) => error.fmt(formatter),
            Self::Metadata(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceRecordResolutionError<E> {}

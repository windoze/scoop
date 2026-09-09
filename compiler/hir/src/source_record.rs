//! Canonical source metadata shared by persistent HIR sections.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedSourcePointRecord {
    byte_offset: u64,
    line: u64,
    column: u64,
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

    /// Verify that every location or span endpoint referenced by a required
    /// semantic section has a canonical point in this record.
    pub fn require_points(
        &self,
        byte_offsets: impl IntoIterator<Item = u64>,
    ) -> Result<(), MissingSourcePointError> {
        for byte_offset in byte_offsets {
            if self.point(byte_offset).is_none() {
                return Err(MissingSourcePointError { byte_offset });
            }
        }
        Ok(())
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSourceRecord {
    identity: scoop_identity::DecodedSourceIdentity,
    byte_length: u64,
    content_digest: scoop_identity::DecodedSourceContentDigest,
    line_starts: Vec<u64>,
    points: Vec<DecodedSourcePointRecord>,
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
        let identity = self
            .identity
            .validate(coordinate)
            .map_err(SourceRecordValidationError::Identity)?;
        let content_digest = self
            .content_digest
            .validate()
            .map_err(SourceRecordValidationError::ContentDigest)?;
        validate_line_starts(&self.line_starts, self.byte_length)?;
        let points = validate_points(&self.points, &self.line_starts, self.byte_length)?;
        let record = SourceRecord {
            identity,
            byte_length: self.byte_length,
            content_digest,
            line_starts: self.line_starts,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingSourcePointError {
    pub byte_offset: u64,
}

impl std::fmt::Display for MissingSourcePointError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "required source point offset {} is missing",
            self.byte_offset
        )
    }
}

impl std::error::Error for MissingSourcePointError {}

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

#[cfg(test)]
mod tests;

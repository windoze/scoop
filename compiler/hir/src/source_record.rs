//! Canonical source metadata shared by persistent HIR sections.

mod validation;
pub use validation::{
    DecodedSourcePointRecord, DecodedSourceRecord, SourceRecordResolutionError,
    SourceRecordValidationError,
};

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

#[cfg(test)]
mod tests;

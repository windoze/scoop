//! Independent Zstandard frames for Scoop-owned archive members.

use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompressionError(String);

impl fmt::Display for CompressionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "slib Zstandard encoding: {}", self.0)
    }
}

impl std::error::Error for CompressionError {}

impl From<std::io::Error> for CompressionError {
    fn from(error: std::io::Error) -> Self {
        Self(error.to_string())
    }
}

pub(crate) fn compress(input: &[u8]) -> Result<Vec<u8>, CompressionError> {
    zstd::bulk::compress(input, -3).map_err(CompressionError::from)
}

pub(crate) fn decompress(input: &[u8]) -> Result<Vec<u8>, CompressionError> {
    let length = zstd::zstd_safe::get_frame_content_size(input)
        .map_err(|_| CompressionError("invalid frame header".to_owned()))?
        .ok_or_else(|| CompressionError("frame has no content size".to_owned()))?;
    let length = usize::try_from(length)
        .map_err(|_| CompressionError("content size does not fit usize".to_owned()))?;
    let frame_length = zstd::zstd_safe::find_frame_compressed_size(input)
        .map_err(|code| CompressionError(zstd::zstd_safe::get_error_name(code).to_owned()))?;
    if frame_length != input.len() {
        return Err(CompressionError("trailing data after frame".to_owned()));
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|error| CompressionError(error.to_string()))?;
    let actual = zstd::bulk::Decompressor::new()?.decompress_to_buffer(input, &mut output)?;
    if actual != length {
        return Err(CompressionError("frame content size mismatch".to_owned()));
    }
    Ok(output)
}

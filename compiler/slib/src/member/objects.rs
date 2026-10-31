//! Encode the retained native objects together after ODR selection.

use super::*;

const MAX_SAMPLES: usize = 2_048;
const SAMPLE_LENGTH: usize = 4_096;
const MIN_SAMPLES: usize = 64;
const MIN_SAMPLE_BYTES: usize = 256 * 1_024;
const DICTIONARY_LENGTH: usize = 64 * 1_024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UncompressedObjectMember {
    content: SlibMemberContent,
    payload: Vec<u8>,
}

impl UncompressedObjectMember {
    pub fn new(
        cone: ConeIdentity,
        stable_key: MemberStableKey,
        role: SlibMemberRole,
        payload: Vec<u8>,
    ) -> Result<Self, SlibMemberRecordError> {
        if !role.is_object() {
            return Err(SlibMemberRecordError::ObjectRoleRequired);
        }
        let content = SlibMemberContent::new(cone, stable_key, role, sha256(&payload))?;
        Ok(Self { content, payload })
    }

    pub const fn content(&self) -> &SlibMemberContent {
        &self.content
    }

    fn encode(
        self,
        compressor: &mut zstd::bulk::Compressor<'_>,
    ) -> Result<SlibMember, SlibMemberRecordError> {
        let stored = compressor
            .compress(&self.payload)
            .map_err(crate::CompressionError::from)
            .map_err(SlibMemberRecordError::Compression)?;
        SlibMember::from_encoded(self.content, stored, Some(self.payload))
    }
}

pub(crate) fn encode_object_members(
    cone: ConeIdentity,
    mut objects: Vec<UncompressedObjectMember>,
) -> Result<Vec<SlibMember>, SlibMemberRecordError> {
    objects.sort_unstable_by_key(|object| object.content.id());
    let dictionary = train_dictionary(&objects);
    let mut compressor = zstd::bulk::Compressor::with_dictionary(3, &dictionary)
        .map_err(crate::CompressionError::from)
        .map_err(SlibMemberRecordError::Compression)?;
    let mut members = Vec::with_capacity(objects.len() + usize::from(!dictionary.is_empty()));
    for object in objects {
        members.push(object.encode(&mut compressor)?);
    }
    if !dictionary.is_empty() {
        members.push(SlibMember::new(
            cone,
            MemberStableKey::ObjectDictionary,
            SlibMemberRole::ObjectDictionary,
            dictionary,
        )?);
    }
    Ok(members)
}

fn train_dictionary(objects: &[UncompressedObjectMember]) -> Vec<u8> {
    let count = objects.len().min(MAX_SAMPLES);
    if count < MIN_SAMPLES {
        return Vec::new();
    }
    let mut samples = Vec::with_capacity(count * SAMPLE_LENGTH);
    let mut lengths = Vec::with_capacity(count);
    for index in 0..count {
        let object = &objects[index * objects.len() / count];
        let length = object.payload.len().min(SAMPLE_LENGTH);
        samples.extend_from_slice(&object.payload[..length]);
        lengths.push(length);
    }
    if samples.len() < MIN_SAMPLE_BYTES {
        return Vec::new();
    }
    // Some small or repetitive corpora cannot train a useful dictionary.
    zstd::dict::from_continuous(&samples, &lengths, DICTIONARY_LENGTH).unwrap_or_default()
}

#[cfg(test)]
mod tests;

//! Closed section and flag matrices for the two built-in object capabilities.

use std::collections::BTreeSet;
use std::fmt;

use object::macho;

use super::ValidatedDarwinArm64ObjectEnvelopeV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinLinkObjectSectionProfileV1 {
    ScoopLir,
    GeneratedCBridge,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuiltinObjectSectionRoleV1 {
    Text,
    ReadOnlyData,
    CString,
    WritableData,
    ZeroFill,
    GccExceptionTable,
    LlvmStackmaps,
    CompactUnwind,
    EhFrame,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedBuiltinObjectSectionInventoryV1 {
    envelope: ValidatedDarwinArm64ObjectEnvelopeV1,
    profile: BuiltinLinkObjectSectionProfileV1,
    roles: Vec<BuiltinObjectSectionRoleV1>,
}

impl ValidatedBuiltinObjectSectionInventoryV1 {
    pub const fn envelope(&self) -> &ValidatedDarwinArm64ObjectEnvelopeV1 {
        &self.envelope
    }

    pub const fn profile(&self) -> BuiltinLinkObjectSectionProfileV1 {
        self.profile
    }

    pub fn roles(&self) -> &[BuiltinObjectSectionRoleV1] {
        &self.roles
    }
}

pub fn validate_builtin_object_section_inventory_v1(
    envelope: ValidatedDarwinArm64ObjectEnvelopeV1,
    profile: BuiltinLinkObjectSectionProfileV1,
) -> Result<ValidatedBuiltinObjectSectionInventoryV1, BuiltinObjectSectionValidationError> {
    if envelope.sections().is_empty() {
        return Err(BuiltinObjectSectionValidationError::EmptySectionTable);
    }
    let mut names = BTreeSet::new();
    let mut roles = Vec::with_capacity(envelope.sections().len());
    for section in envelope.sections() {
        let name = (section.segment_name(), section.section_name());
        if !names.insert((name.0.to_vec(), name.1.to_vec())) {
            return Err(BuiltinObjectSectionValidationError::DuplicateSectionName);
        }
        let Some((role, expected_flags)) = classify_section(name.0, name.1) else {
            return Err(BuiltinObjectSectionValidationError::UnsupportedSectionName);
        };
        if !section_flags_match(role, expected_flags, section.flags()) {
            return Err(BuiltinObjectSectionValidationError::SectionFlagsMismatch {
                role,
                expected: expected_flags,
                actual: section.flags(),
            });
        }
        if !profile_accepts(profile, role) {
            return Err(
                BuiltinObjectSectionValidationError::UnsupportedSectionForProfile { profile, role },
            );
        }
        roles.push(role);
    }
    if profile == BuiltinLinkObjectSectionProfileV1::GeneratedCBridge
        && !roles.contains(&BuiltinObjectSectionRoleV1::Text)
    {
        return Err(BuiltinObjectSectionValidationError::MissingGeneratedBridgeText);
    }
    Ok(ValidatedBuiltinObjectSectionInventoryV1 {
        envelope,
        profile,
        roles,
    })
}

fn section_flags_match(role: BuiltinObjectSectionRoleV1, expected: u32, actual: u32) -> bool {
    actual == expected
        || (role == BuiltinObjectSectionRoleV1::Text
            && actual == macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS)
}

fn classify_section(segment: &[u8], section: &[u8]) -> Option<(BuiltinObjectSectionRoleV1, u32)> {
    match (segment, section) {
        (b"__TEXT", b"__text") => Some((
            BuiltinObjectSectionRoleV1::Text,
            macho::S_REGULAR | macho::S_ATTR_PURE_INSTRUCTIONS | macho::S_ATTR_SOME_INSTRUCTIONS,
        )),
        (b"__TEXT", b"__const") | (b"__DATA", b"__const") | (b"__DATA_CONST", b"__const") => {
            Some((BuiltinObjectSectionRoleV1::ReadOnlyData, macho::S_REGULAR))
        }
        (b"__TEXT", b"__cstring") => Some((
            BuiltinObjectSectionRoleV1::CString,
            macho::S_CSTRING_LITERALS,
        )),
        (b"__DATA", b"__data") => {
            Some((BuiltinObjectSectionRoleV1::WritableData, macho::S_REGULAR))
        }
        (b"__DATA", b"__bss") => Some((BuiltinObjectSectionRoleV1::ZeroFill, macho::S_ZEROFILL)),
        (b"__TEXT", b"__gcc_except_tab") => Some((
            BuiltinObjectSectionRoleV1::GccExceptionTable,
            macho::S_REGULAR,
        )),
        (b"__LLVM_STACKMAPS", b"__llvm_stackmaps") => {
            Some((BuiltinObjectSectionRoleV1::LlvmStackmaps, macho::S_REGULAR))
        }
        (b"__LD", b"__compact_unwind") => Some((
            BuiltinObjectSectionRoleV1::CompactUnwind,
            macho::S_REGULAR | macho::S_ATTR_DEBUG,
        )),
        (b"__TEXT", b"__eh_frame") => Some((
            BuiltinObjectSectionRoleV1::EhFrame,
            macho::S_COALESCED
                | macho::S_ATTR_NO_TOC
                | macho::S_ATTR_STRIP_STATIC_SYMS
                | macho::S_ATTR_LIVE_SUPPORT,
        )),
        _ => None,
    }
}

fn profile_accepts(
    profile: BuiltinLinkObjectSectionProfileV1,
    role: BuiltinObjectSectionRoleV1,
) -> bool {
    match profile {
        BuiltinLinkObjectSectionProfileV1::ScoopLir => true,
        BuiltinLinkObjectSectionProfileV1::GeneratedCBridge => matches!(
            role,
            BuiltinObjectSectionRoleV1::Text
                | BuiltinObjectSectionRoleV1::ReadOnlyData
                | BuiltinObjectSectionRoleV1::CString
                | BuiltinObjectSectionRoleV1::CompactUnwind
                | BuiltinObjectSectionRoleV1::EhFrame
        ),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuiltinObjectSectionValidationError {
    EmptySectionTable,
    DuplicateSectionName,
    UnsupportedSectionName,
    SectionFlagsMismatch {
        role: BuiltinObjectSectionRoleV1,
        expected: u32,
        actual: u32,
    },
    UnsupportedSectionForProfile {
        profile: BuiltinLinkObjectSectionProfileV1,
        role: BuiltinObjectSectionRoleV1,
    },
    MissingGeneratedBridgeText,
}

impl fmt::Display for BuiltinObjectSectionValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid built-in object section inventory: {self:?}"
        )
    }
}

impl std::error::Error for BuiltinObjectSectionValidationError {}

#[cfg(test)]
mod tests;

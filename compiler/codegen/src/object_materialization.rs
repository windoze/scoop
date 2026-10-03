//! Resolve typed runtime-metadata patch locations into object file offsets.

use std::collections::BTreeSet;
use std::path::Path;

use object::{Object, ObjectSection, ObjectSymbol};

use crate::{CodegenError, EmittedStrongRuntimeMetadataV1, ProvisionalStrongDigestPatchLocationV1};

/// One typed digest patch location resolved against the exact provisional
/// object bytes emitted by LLVM.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmittedStrongDigestPatchMaterializationV1 {
    location: ProvisionalStrongDigestPatchLocationV1,
    checked_object_offset: u64,
}

impl EmittedStrongDigestPatchMaterializationV1 {
    pub const fn location(self) -> ProvisionalStrongDigestPatchLocationV1 {
        self.location
    }

    pub const fn checked_object_offset(self) -> u64 {
        self.checked_object_offset
    }
}

pub(crate) fn resolve_digest_patch_materializations_v1(
    path: &Path,
    target: scoop_lir::LirTargetProfile,
    canonical_definitions: &scoop_lir::ObjectSymbolSurfaceV1,
    metadata: &EmittedStrongRuntimeMetadataV1,
) -> Result<Vec<EmittedStrongDigestPatchMaterializationV1>, CodegenError> {
    let bytes = std::fs::read(path).map_err(|error| {
        CodegenError(format!(
            "cannot read emitted object {} for patch materialization: {error}",
            path.display()
        ))
    })?;
    let object = object::File::parse(bytes.as_slice()).map_err(|error| {
        CodegenError(format!(
            "cannot parse emitted object {} for patch materialization: {error}",
            path.display()
        ))
    })?;
    let normalization = target.contract().native_symbol_normalization();
    let mut offsets = BTreeSet::new();
    let mut materializations = Vec::with_capacity(metadata.patch_locations().len());
    for location in metadata.patch_locations() {
        let owner = location.owner().symbol();
        let macho_name = normalization.compiler_generated_object_symbol(owner.as_str());
        let symbol = object.symbol_by_name(&macho_name).ok_or_else(|| {
            CodegenError(format!(
                "digest patch owner `{owner}` is absent from emitted object {}",
                path.display()
            ))
        })?;
        if !symbol.is_definition() {
            return Err(CodegenError(format!(
                "digest patch owner `{owner}` is not defined in emitted object {}",
                path.display()
            )));
        }
        let section_index = symbol.section_index().ok_or_else(|| {
            CodegenError(format!(
                "digest patch owner `{owner}` has no concrete section in emitted object {}",
                path.display()
            ))
        })?;
        let section = object.section_by_index(section_index).map_err(|error| {
            CodegenError(format!(
                "cannot resolve section for digest patch owner `{owner}`: {error}"
            ))
        })?;
        let (section_file_offset, section_file_size) = section.file_range().ok_or_else(|| {
            CodegenError(format!(
                "digest patch owner `{owner}` belongs to a section without file bytes"
            ))
        })?;
        let owner_offset = symbol
            .address()
            .checked_sub(section.address())
            .ok_or_else(|| {
                CodegenError(format!(
                    "digest patch owner `{owner}` precedes its containing section"
                ))
            })?;
        let definition = canonical_definitions
            .plan(location.definition())
            .ok_or_else(|| {
                CodegenError(format!(
                    "digest patch owner `{owner}` references an unknown strong definition"
                ))
            })?;
        let end_request = definition
            .atom_boundaries()
            .iter()
            .find(|boundary| boundary.atom() == location.atom())
            .map(|boundary| boundary.end())
            .ok_or_else(|| {
                CodegenError(format!(
                    "digest patch owner `{owner}` has no planned atom boundary"
                ))
            })?;
        let end_name =
            normalization.compiler_generated_object_symbol(end_request.symbol().as_str());
        let end_symbol = object.symbol_by_name(&end_name).ok_or_else(|| {
            CodegenError(format!(
                "digest patch owner `{owner}` has no emitted end boundary `{}`",
                end_request.symbol()
            ))
        })?;
        if !end_symbol.is_definition() || end_symbol.section_index() != Some(section_index) {
            return Err(CodegenError(format!(
                "digest patch owner `{owner}` end boundary is not defined in the owner section"
            )));
        }
        let owner_extent = end_symbol
            .address()
            .checked_sub(symbol.address())
            .ok_or_else(|| {
                CodegenError(format!(
                    "digest patch owner `{owner}` end boundary precedes the owner"
                ))
            })?;
        let patch_end_within_owner = location
            .offset_within_owner()
            .checked_add(u64::from(location.width_bytes()))
            .ok_or_else(|| CodegenError(format!("digest patch owner `{owner}` range overflows")))?;
        if patch_end_within_owner > owner_extent {
            return Err(CodegenError(format!(
                "digest patch in `{owner}` ends at {}, beyond owner size {}",
                patch_end_within_owner, owner_extent
            )));
        }
        let checked_object_offset = section_file_offset
            .checked_add(owner_offset)
            .and_then(|offset| offset.checked_add(location.offset_within_owner()))
            .ok_or_else(|| {
                CodegenError(format!("digest patch owner `{owner}` offset overflows"))
            })?;
        let checked_object_end = checked_object_offset
            .checked_add(u64::from(location.width_bytes()))
            .ok_or_else(|| CodegenError(format!("digest patch owner `{owner}` end overflows")))?;
        let section_file_end = section_file_offset
            .checked_add(section_file_size)
            .ok_or_else(|| {
                CodegenError(format!("digest patch owner `{owner}` section overflows"))
            })?;
        if checked_object_end > section_file_end {
            return Err(CodegenError(format!(
                "digest patch in `{owner}` escapes its section file range"
            )));
        }
        let start = usize::try_from(checked_object_offset).map_err(|_| {
            CodegenError(format!("digest patch owner `{owner}` offset exceeds usize"))
        })?;
        let end = usize::try_from(checked_object_end)
            .map_err(|_| CodegenError(format!("digest patch owner `{owner}` end exceeds usize")))?;
        let slot = bytes.get(start..end).ok_or_else(|| {
            CodegenError(format!(
                "digest patch in `{owner}` escapes emitted object bytes"
            ))
        })?;
        if slot.iter().any(|byte| *byte != 0) {
            return Err(CodegenError(format!(
                "digest patch in `{owner}` is not provisionally zero"
            )));
        }
        if !offsets.insert((checked_object_offset, checked_object_end)) {
            return Err(CodegenError(format!(
                "digest patch in `{owner}` repeats an object byte range"
            )));
        }
        materializations.push(EmittedStrongDigestPatchMaterializationV1 {
            location: *location,
            checked_object_offset,
        });
    }
    for pair in offsets.iter().copied().collect::<Vec<_>>().windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(CodegenError(format!(
                "digest patch object ranges {}..{} and {}..{} overlap",
                pair[0].0, pair[0].1, pair[1].0, pair[1].1
            )));
        }
    }
    Ok(materializations)
}

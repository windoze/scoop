//! Frame and instruction rules, independent of the object container and libc.

use std::ops::Range;

use crate::CodegenError;
use crate::target::CodeArchitecture;

use super::{FunctionRelocation, TextSection, aarch64, x86_64};

impl CodeArchitecture {
    pub(super) fn instruction_alignment(self) -> u64 {
        match self {
            Self::Aarch64 => 4,
            Self::X86_64 => 1,
        }
    }

    pub(super) fn instruction_starts(self, text: &TextSection) -> Result<Vec<u64>, CodegenError> {
        match self {
            Self::Aarch64 => Ok((0..text.bytes.len())
                .step_by(4)
                .map(|offset| text.address + offset as u64)
                .collect()),
            Self::X86_64 => Ok(x86_64::instructions(&text.bytes, text.address)?
                .into_iter()
                .map(|instruction| instruction.start)
                .collect()),
        }
    }

    pub(super) fn valid_stack_size(self, size: u64) -> bool {
        match self {
            Self::Aarch64 => size >= 16 && size % 16 == 0,
            Self::X86_64 => size >= 8 && size % 16 == 8 && size != u64::MAX,
        }
    }

    pub(super) fn root_offset(self, register: u16, offset: i32, size: u64) -> Option<i128> {
        let (sp, fp, saved) = match self {
            Self::Aarch64 => (31, 29, 16),
            Self::X86_64 => (7, 6, 8),
        };
        let spill_end = i128::from(size) - saved;
        let base = if register == sp {
            0
        } else if register == fp {
            spill_end
        } else {
            return None;
        };
        let result = base + i128::from(offset);
        (result >= 0 && result % 8 == 0 && result + 8 <= spill_end).then_some(result)
    }

    pub(super) fn validate_safepoint(
        self,
        text: &TextSection,
        function: &FunctionRelocation,
        instruction_offset: u32,
        safepoint: u64,
        check_frame: bool,
    ) -> Result<u64, CodegenError> {
        match self {
            Self::Aarch64 => {
                let return_pc = aarch64::validate_aarch64_return_pc(
                    text,
                    function,
                    instruction_offset,
                    safepoint,
                )?;
                if check_frame {
                    aarch64::validate_aarch64_frame_chain(text, function, return_pc)?;
                }
                Ok(return_pc - 4)
            }
            Self::X86_64 => x86_64::validate_safepoint(
                text,
                function,
                instruction_offset,
                safepoint,
                check_frame,
            ),
        }
    }

    pub(super) fn calls(
        self,
        text: &TextSection,
        range: Range<u64>,
    ) -> Result<Vec<u64>, CodegenError> {
        let bytes = text.range(range.clone())?;
        match self {
            Self::Aarch64 => {
                if range.start % 4 != 0 || bytes.len() % 4 != 0 {
                    return Err(CodegenError(
                        "AArch64 protected range is not instruction-aligned".into(),
                    ));
                }
                Ok(bytes
                    .chunks_exact(4)
                    .enumerate()
                    .filter_map(|(index, instruction)| {
                        aarch64::is_aarch64_call(u32::from_le_bytes(
                            instruction.try_into().expect("instruction width"),
                        ))
                        .then_some(range.start + index as u64 * 4)
                    })
                    .collect())
            }
            Self::X86_64 => Ok(x86_64::instructions(bytes, range.start)?
                .into_iter()
                .filter(|instruction| instruction.call)
                .map(|instruction| instruction.start)
                .collect()),
        }
    }
}

impl TextSection {
    pub(super) fn range(&self, range: Range<u64>) -> Result<&[u8], CodegenError> {
        let offsets = range
            .start
            .checked_sub(self.address)
            .zip(range.end.checked_sub(self.address))
            .and_then(|(start, end)| {
                Some((usize::try_from(start).ok()?, usize::try_from(end).ok()?))
            });
        offsets
            .and_then(|(start, end)| self.bytes.get(start..end))
            .ok_or_else(|| {
                CodegenError(format!(
                    "code range {range:?} lies outside function/section at {:#x}",
                    self.address
                ))
            })
    }
}

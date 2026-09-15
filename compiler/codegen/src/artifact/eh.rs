//! Complete-LIR EH expectations and Darwin/AArch64 object qualification.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use scoop_lir::{Instruction, Module};

use crate::CodegenError;
use crate::target::LsdaEncodingProfile;

use super::aarch64::is_aarch64_call;
use super::{ObservedSafepoints, TextSection};

mod cursor;
mod frame;
mod lsda;

use frame::parse_eh_frame;
use lsda::parse_lsda;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum EhActionKind {
    Cleanup,
    CatchAll,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExpectedEhFunction {
    invokes: Vec<ExpectedEhInvoke>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ExpectedEhInvoke {
    action: EhActionKind,
    safepoint: Option<u64>,
    identity: String,
}

impl ExpectedEhFunction {
    fn insert(&mut self, action: EhActionKind, safepoint: Option<u64>, identity: String) {
        self.invokes.push(ExpectedEhInvoke {
            action,
            safepoint,
            identity,
        });
    }

    fn actions(&self) -> BTreeSet<EhActionKind> {
        self.invokes.iter().map(|invoke| invoke.action).collect()
    }

    #[cfg(test)]
    fn action_count(&self, action: EhActionKind) -> usize {
        self.invokes
            .iter()
            .filter(|invoke| invoke.action == action)
            .count()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ExpectedEh {
    functions: BTreeMap<String, ExpectedEhFunction>,
}

impl ExpectedEh {
    pub(crate) fn function_count(&self) -> usize {
        self.functions.len()
    }

    fn function(&self, symbol: &str) -> Option<&ExpectedEhFunction> {
        self.functions.get(symbol)
    }

    pub(crate) fn without_body_metadata(&self) -> Self {
        Self::default()
    }

    pub(crate) fn for_function(&self, symbol: &str) -> Self {
        Self {
            functions: self
                .functions
                .get(symbol)
                .cloned()
                .map(|function| BTreeMap::from([(symbol.to_string(), function)]))
                .unwrap_or_default(),
        }
    }

    #[cfg(test)]
    pub(crate) fn action_count(&self, symbol: &str, action: EhActionKind) -> usize {
        self.function(symbol)
            .map_or(0, |function| function.action_count(action))
    }
}

/// Build the object-level EH manifest from explicit invoke unwind edges.
///
/// Merely finding a detached landing-pad block is not enough: LLVM is free to
/// discard unreachable blocks before object emission. Every expectation is
/// therefore rooted in an `Invoke` and classified by that invoke's unwind
/// destination in complete LIR.
pub(crate) fn expectations(module: &Module) -> Result<ExpectedEh, CodegenError> {
    let mut functions = BTreeMap::new();
    let mut symbols = BTreeSet::new();
    let mut managed_safepoints = BTreeSet::new();
    for function in &module.functions {
        if !symbols.insert(function.symbol()) {
            return Err(CodegenError(format!(
                "duplicate LIR function symbol `{}` in EH manifest",
                function.symbol()
            )));
        }
        let mut expectation = ExpectedEhFunction::default();
        for (_, block) in function.blocks.iter() {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let Instruction::Invoke { site } = instruction else {
                    continue;
                };
                let unwind = &function.blocks[site.unwind()];
                let action = match unwind.instructions.first() {
                    Some(Instruction::LandingPad { .. }) => EhActionKind::CatchAll,
                    Some(Instruction::CleanupPad { .. }) => EhActionKind::Cleanup,
                    _ => {
                        return Err(CodegenError(format!(
                            "invoke @{} block `{}` unwinds to `{}`, which does not begin with an EH pad",
                            function.symbol(),
                            block.name,
                            unwind.name
                        )));
                    }
                };
                let pad_count = unwind
                    .instructions
                    .iter()
                    .filter(|instruction| {
                        matches!(
                            instruction,
                            Instruction::LandingPad { .. } | Instruction::CleanupPad { .. }
                        )
                    })
                    .count();
                if pad_count != 1 {
                    return Err(CodegenError(format!(
                        "invoke @{} block `{}` unwind destination `{}` contains {pad_count} EH pads",
                        function.symbol(),
                        block.name,
                        unwind.name
                    )));
                }
                let safepoint = match site {
                    scoop_lir::InvokeSite::Managed(site) => {
                        let id = function
                            .safepoints
                            .get(site.safepoint)
                            .ok_or_else(|| {
                                CodegenError(format!(
                                    "managed invoke in `{}` references missing safepoint site {}",
                                    function.symbol(),
                                    site.safepoint.into_u32()
                                ))
                            })?
                            .runtime_id()
                            .get();
                        if !managed_safepoints.insert(id) {
                            return Err(CodegenError(format!(
                                "duplicate managed invoke SafepointId {id} in EH manifest"
                            )));
                        }
                        Some(id)
                    }
                    scoop_lir::InvokeSite::NoGc(_) => None,
                };
                expectation.insert(
                    action,
                    safepoint,
                    format!("block `{}` instruction {instruction_index}", block.name),
                );
            }
        }
        if !expectation.invokes.is_empty() {
            functions.insert(function.symbol().to_string(), expectation);
        }
    }
    Ok(ExpectedEh { functions })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EhRelocation {
    pub(super) offset: u64,
    pub(super) r_type: u8,
    pub(super) r_pcrel: bool,
    pub(super) r_length: u8,
    pub(super) symbol: String,
    pub(super) symbol_address: u64,
    pub(super) symbol_section: Option<String>,
    pub(super) symbol_is_undefined: bool,
    pub(super) symbol_is_local: bool,
    pub(super) symbol_is_text: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct EhSection {
    pub(super) address: u64,
    pub(super) bytes: Vec<u8>,
    pub(super) relocations: Vec<EhRelocation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Fde {
    function_symbol: String,
    function_start: u64,
    function_size: u64,
    lsda_address: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ObservedLsda {
    actions: BTreeSet<EhActionKind>,
    protected_ranges: Vec<ObservedProtectedRange>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ObservedProtectedRange {
    range: Range<u64>,
    action: EhActionKind,
}

/// Verify all object EH structures after the Mach-O layer has extracted the
/// three relevant sections and preserved their relocation targets.
pub(super) fn verify_sections(
    eh_frame: Option<&EhSection>,
    gcc_except_tab: Option<&EhSection>,
    text: Option<&TextSection>,
    expected: &ExpectedEh,
    observed_safepoints: &ObservedSafepoints,
    encodings: LsdaEncodingProfile,
) -> Result<(), CodegenError> {
    let parsed_eh_frame = eh_frame.map(parse_eh_frame).transpose()?;
    if let Some(frame) = &parsed_eh_frame {
        let text = text.ok_or_else(|| {
            CodegenError("emitted Mach-O has __eh_frame FDEs but no __text section".to_string())
        })?;
        validate_function_ranges(&frame.scoop_fdes, &frame.unwind_only_fdes, text)?;
    }
    if expected.function_count() == 0 {
        if gcc_except_tab.is_some()
            || parsed_eh_frame
                .as_ref()
                .is_some_and(|frame| !frame.scoop_fdes.is_empty())
        {
            return Err(CodegenError(
                "emitted Mach-O contains Scoop personality/LSDA metadata but complete LIR has no invoke unwind edges"
                    .to_string(),
            ));
        }
        return Ok(());
    }
    let parsed_eh_frame = parsed_eh_frame.ok_or_else(|| {
        CodegenError("emitted Mach-O has EH functions but no __eh_frame section".to_string())
    })?;
    let gcc_except_tab = gcc_except_tab.ok_or_else(|| {
        CodegenError("emitted Mach-O has EH functions but no __gcc_except_tab section".to_string())
    })?;
    let text = text.ok_or_else(|| {
        CodegenError("emitted Mach-O has EH functions but no __text section".to_string())
    })?;
    if !gcc_except_tab.relocations.is_empty() {
        return Err(CodegenError(format!(
            "__gcc_except_tab carries {} relocations; the closed null catch-all profile requires none",
            gcc_except_tab.relocations.len()
        )));
    }

    let mut fdes = parsed_eh_frame.scoop_fdes;
    fdes.sort_by_key(|fde| fde.lsda_address);
    if fdes.len() != expected.function_count() {
        return Err(CodegenError(format!(
            "__eh_frame contains {} Scoop FDEs, complete LIR requires {}",
            fdes.len(),
            expected.function_count()
        )));
    }
    if fdes
        .first()
        .is_none_or(|fde| fde.lsda_address != gcc_except_tab.address)
    {
        return Err(CodegenError(
            "first FDE LSDA does not start at the beginning of __gcc_except_tab".to_string(),
        ));
    }

    let section_end =
        gcc_except_tab
            .address
            .checked_add(u64::try_from(gcc_except_tab.bytes.len()).map_err(|_| {
                CodegenError("__gcc_except_tab length exceeds u64::MAX".to_string())
            })?)
            .ok_or_else(|| CodegenError("__gcc_except_tab address range overflows".to_string()))?;
    let mut seen_functions = BTreeSet::new();
    for (index, fde) in fdes.iter().enumerate() {
        let lir_symbol = fde.function_symbol.strip_prefix('_').ok_or_else(|| {
            CodegenError(format!(
                "__eh_frame function symbol `{}` lacks the Mach-O global prefix",
                fde.function_symbol
            ))
        })?;
        let expectation = expected.function(lir_symbol).ok_or_else(|| {
            CodegenError(format!(
                "__eh_frame contains unexpected function FDE `{}`",
                fde.function_symbol
            ))
        })?;
        if !seen_functions.insert(lir_symbol.to_string()) {
            return Err(CodegenError(format!(
                "__eh_frame repeats function FDE `{}`",
                fde.function_symbol
            )));
        }
        let next = fdes
            .get(index + 1)
            .map_or(section_end, |next| next.lsda_address);
        if fde.lsda_address < gcc_except_tab.address
            || fde.lsda_address >= section_end
            || next <= fde.lsda_address
            || next > section_end
        {
            return Err(CodegenError(format!(
                "FDE `{}` has LSDA address range {:#x}..{next:#x} outside __gcc_except_tab {:#x}..{section_end:#x}",
                fde.function_symbol, fde.lsda_address, gcc_except_tab.address
            )));
        }
        let start = usize::try_from(fde.lsda_address - gcc_except_tab.address)
            .expect("section-relative address fits its byte-vector length");
        let end = usize::try_from(next - gcc_except_tab.address)
            .expect("section-relative address fits its byte-vector length");
        let observed = parse_lsda(
            &gcc_except_tab.bytes[start..end],
            fde.function_size,
            encodings,
        )
        .map_err(|error| {
            CodegenError(format!(
                "LSDA for `{}` violates the closed profile: {error}",
                fde.function_symbol
            ))
        })?;
        let expected_actions = expectation.actions();
        if observed.actions != expected_actions {
            return Err(CodegenError(format!(
                "LSDA actions for `{}` disagree with complete LIR: expected {:?}, observed {:?}",
                fde.function_symbol, expected_actions, observed.actions
            )));
        }
        validate_protected_calls(&observed, fde, text, expectation, observed_safepoints)?;
    }
    Ok(())
}

fn validate_protected_calls(
    observed: &ObservedLsda,
    fde: &Fde,
    text: &TextSection,
    expected: &ExpectedEhFunction,
    observed_safepoints: &ObservedSafepoints,
) -> Result<(), CodegenError> {
    let expected_managed = expected
        .invokes
        .iter()
        .filter_map(|invoke| invoke.safepoint.map(|id| (id, invoke)))
        .collect::<BTreeMap<_, _>>();
    let mut safepoints_by_pc = BTreeMap::new();
    for (id, site) in &observed_safepoints.sites {
        if let Some(previous) = safepoints_by_pc.insert(site.call_pc, (*id, site)) {
            return Err(CodegenError(format!(
                "stackmap SafepointIds {} and {id} share call PC {:#x}",
                previous.0, site.call_pc
            )));
        }
    }
    let mut seen_managed = BTreeSet::new();
    let mut observed_no_gc = BTreeMap::<EhActionKind, usize>::new();
    let function_start = usize::try_from(fde.function_start - text.address)
        .expect("validated function start fits the __text byte vector");
    for protected in &observed.protected_ranges {
        let range = &protected.range;
        let start = function_start
            .checked_add(usize::try_from(range.start).expect("call-site offset fits usize"))
            .ok_or_else(|| CodegenError("protected call-site start overflows usize".to_string()))?;
        let end = function_start
            .checked_add(usize::try_from(range.end).expect("call-site offset fits usize"))
            .ok_or_else(|| CodegenError("protected call-site end overflows usize".to_string()))?;
        let bytes = text.bytes.get(start..end).ok_or_else(|| {
            CodegenError(format!(
                "protected call-site range {:?} for `{}` lies outside __text",
                range, fde.function_symbol
            ))
        })?;
        let mut call_count = 0usize;
        for (index, instruction) in bytes.chunks_exact(4).enumerate() {
            if !is_aarch64_call(u32::from_le_bytes(
                instruction.try_into().expect("four-byte instruction"),
            )) {
                continue;
            }
            call_count += 1;
            let byte_offset = u64::try_from(index)
                .expect("instruction index fits u64")
                .checked_mul(4)
                .ok_or_else(|| CodegenError("protected call offset overflows".to_string()))?;
            let call_pc = fde
                .function_start
                .checked_add(range.start)
                .and_then(|address| address.checked_add(byte_offset))
                .ok_or_else(|| CodegenError("protected call PC overflows".to_string()))?;
            let Some((safepoint, site)) = safepoints_by_pc.get(&call_pc).copied() else {
                *observed_no_gc.entry(protected.action).or_default() += 1;
                continue;
            };
            let invoke = expected_managed.get(&safepoint).ok_or_else(|| {
                CodegenError(format!(
                    "LSDA protects unexpected managed SafepointId {safepoint} at {call_pc:#x}"
                ))
            })?;
            if site.function_symbol != fde.function_symbol {
                return Err(CodegenError(format!(
                    "managed invoke SafepointId {safepoint} belongs to `{}`, but its LSDA FDE is `{}`",
                    site.function_symbol, fde.function_symbol
                )));
            }
            if invoke.action != protected.action {
                return Err(CodegenError(format!(
                    "managed invoke SafepointId {safepoint} ({}) expects {:?}, but LSDA records {:?}",
                    invoke.identity, invoke.action, protected.action
                )));
            }
            if !seen_managed.insert(safepoint) {
                return Err(CodegenError(format!(
                    "managed invoke SafepointId {safepoint} is covered by more than one LSDA range"
                )));
            }
        }
        if call_count == 0 {
            return Err(CodegenError(format!(
                "protected call-site range {:?} for `{}` contains no AArch64 bl/blr instruction",
                range, fde.function_symbol
            )));
        }
    }
    let missing = expected_managed
        .keys()
        .filter(|id| !seen_managed.contains(id))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(CodegenError(format!(
            "LSDA for `{}` does not cover managed invoke SafepointIds {missing:?}",
            fde.function_symbol
        )));
    }
    for action in [EhActionKind::Cleanup, EhActionKind::CatchAll] {
        let expected_count = expected
            .invokes
            .iter()
            .filter(|invoke| invoke.safepoint.is_none() && invoke.action == action)
            .count();
        let observed_count = observed_no_gc.get(&action).copied().unwrap_or(0);
        if observed_count != expected_count {
            return Err(CodegenError(format!(
                "LSDA for `{}` covers {observed_count} non-statepoint {:?} calls, complete LIR requires {expected_count}",
                fde.function_symbol, action
            )));
        }
    }
    Ok(())
}

fn validate_function_ranges(
    scoop_fdes: &[Fde],
    unwind_only_fdes: &[frame::UnwindOnlyFde],
    text: &TextSection,
) -> Result<(), CodegenError> {
    let text_end = text
        .address
        .checked_add(
            u64::try_from(text.bytes.len())
                .map_err(|_| CodegenError("Mach-O __text length exceeds u64::MAX".to_string()))?,
        )
        .ok_or_else(|| CodegenError("Mach-O __text address range overflows".to_string()))?;
    let mut ranges = Vec::with_capacity(scoop_fdes.len() + unwind_only_fdes.len());
    for (symbol, start, size) in scoop_fdes
        .iter()
        .map(|fde| {
            (
                fde.function_symbol.as_str(),
                fde.function_start,
                fde.function_size,
            )
        })
        .chain(unwind_only_fdes.iter().map(|fde| {
            (
                fde.function_symbol.as_str(),
                fde.function_start,
                fde.function_size,
            )
        }))
    {
        let end = start
            .checked_add(size)
            .ok_or_else(|| CodegenError(format!("FDE `{symbol}` function range overflows")))?;
        if size == 0 || start % 4 != 0 || size % 4 != 0 || start < text.address || end > text_end {
            return Err(CodegenError(format!(
                "FDE `{symbol}` function range {start:#x}..{end:#x} lies outside __text {:#x}..{text_end:#x}",
                text.address
            )));
        }
        ranges.push((start, end, symbol));
    }
    ranges.sort_unstable_by_key(|range| range.0);
    for pair in ranges.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(CodegenError(format!(
                "FDE function ranges for `{}` and `{}` overlap",
                pair[0].2, pair[1].2
            )));
        }
    }
    Ok(())
}

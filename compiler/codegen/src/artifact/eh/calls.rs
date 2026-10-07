//! Match protected machine calls to the existing LIR invoke manifest.

use super::lsda::parse_lsda;
use super::*;
use crate::target::CodeArchitecture;

pub(super) fn verify_lsda(
    bytes: &[u8],
    fde: &Fde,
    text: &TextSection,
    expectation: &ExpectedEhFunction,
    observed_safepoints: &ObservedSafepoints,
    encodings: LsdaEncodingProfile,
    architecture: CodeArchitecture,
) -> Result<(), CodegenError> {
    let observed =
        parse_lsda(bytes, fde.function_size, encodings, architecture).map_err(|error| {
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
    validate_protected_calls(
        &observed,
        fde,
        text,
        expectation,
        observed_safepoints,
        architecture,
    )
}

fn validate_protected_calls(
    observed: &ObservedLsda,
    fde: &Fde,
    text: &TextSection,
    expected: &ExpectedEhFunction,
    observed_safepoints: &ObservedSafepoints,
    architecture: CodeArchitecture,
) -> Result<(), CodegenError> {
    let expected_managed = expected
        .invokes
        .iter()
        .filter_map(|invoke| invoke.safepoint.map(|id| (id, invoke)))
        .collect::<BTreeMap<_, _>>();
    let mut safepoints_by_pc = BTreeMap::new();
    for (id, site) in &observed_safepoints.sites {
        if site.function_symbol != fde.function_symbol {
            continue;
        }
        if let Some(previous) = safepoints_by_pc.insert(site.call_pc, (*id, site)) {
            return Err(CodegenError(format!(
                "stackmap SafepointIds {} and {id} share call PC {:#x}",
                previous.0, site.call_pc
            )));
        }
    }
    let mut seen_managed = BTreeSet::new();
    let mut observed_no_gc = BTreeMap::<EhActionKind, usize>::new();
    let instruction_starts = architecture.instruction_starts(text)?;
    for protected in &observed.protected_ranges {
        let landing_pad = fde
            .function_start
            .checked_add(protected.landing_pad)
            .ok_or_else(|| CodegenError("landing-pad address overflows".into()))?;
        if instruction_starts.binary_search(&landing_pad).is_err() {
            return Err(CodegenError(format!(
                "LSDA landing pad {landing_pad:#x} is not an instruction boundary"
            )));
        }
        let range = &protected.range;
        let start = fde
            .function_start
            .checked_add(range.start)
            .ok_or_else(|| CodegenError("protected range start overflows".into()))?;
        let end = fde
            .function_start
            .checked_add(range.end)
            .ok_or_else(|| CodegenError("protected range end overflows".into()))?;
        let calls = architecture.calls(text, start..end)?;
        let call_count = calls.len();
        for call_pc in calls {
            let Some((safepoint, site)) = safepoints_by_pc.get(&call_pc).copied() else {
                if !text.non_unwinding_calls.contains(&call_pc) {
                    *observed_no_gc.entry(protected.action).or_default() += 1;
                }
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
                "protected call-site range {:?} for `{}` contains no call instruction",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ObservedSafepoint;

    #[test]
    fn backend_copy_in_protected_range_keeps_managed_invoke_checks() {
        let fde = Fde {
            function_symbol: "copy_and_invoke".into(),
            function_start: 0,
            function_size: 16,
            lsda_address: 0,
        };
        let observed = ObservedLsda {
            actions: BTreeSet::from([EhActionKind::Cleanup]),
            protected_ranges: vec![ObservedProtectedRange {
                range: 0..8,
                action: EhActionKind::Cleanup,
                landing_pad: 12,
            }],
        };
        let mut expected = ExpectedEhFunction::default();
        expected.insert(EhActionKind::Cleanup, Some(7), "managed invoke".into());
        let mut sites = ObservedSafepoints {
            sites: BTreeMap::from([(
                7,
                ObservedSafepoint {
                    function_symbol: fde.function_symbol.clone(),
                    call_pc: 4,
                },
            )]),
        };
        let mut text = TextSection {
            address: 0,
            bytes: [0x9400_0000u32, 0x9400_0000, 0, 0]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect(),
            non_unwinding_calls: BTreeSet::from([0]),
        };
        let verify = |text: &TextSection, sites: &ObservedSafepoints| {
            validate_protected_calls(
                &observed,
                &fde,
                text,
                &expected,
                sites,
                CodeArchitecture::Aarch64,
            )
        };
        verify(&text, &sites).expect("memcpy does not add an unwind edge");
        text.non_unwinding_calls.clear();
        assert!(
            verify(&text, &sites).is_err(),
            "unknown extra call rejected"
        );
        text.non_unwinding_calls.extend([0, 4]);
        sites.sites.clear();
        assert!(
            verify(&text, &sites).is_err(),
            "managed invoke still required"
        );
    }
}

/// Mode-specific lowering decisions derived from the full suspension-aware plan.
///
/// This is the first migration layer for T2003u3a: it keeps the complete
/// `HandleStateMachinePlan` as the source of truth, then records which parts
/// can be lowered as flag-unwind, stack re-entry, or heap continuation
/// materialization without re-scanning source shapes.
#[derive(Debug, Clone)]
pub(super) struct HandleModeSpecificSimplification {
    frame_requirements: SimplifiedFrameRequirements,
    cleanup_strategy: SimplifiedCleanupStrategy,
    suspend_sites: Vec<SimplifiedSuspendSite>,
    arm_summaries: Vec<SimplifiedArmSummary>,
    nested_handles: Vec<HandleModeSpecificSimplification>,
}

impl HandleModeSpecificSimplification {
    fn from_full_plan(plan: &HandleStateMachinePlan) -> Self {
        Self {
            frame_requirements: SimplifiedFrameRequirements {
                uses_shared_payload_transport: !plan.suspend_sites.is_empty(),
                needs_stack_reentry: plan
                    .arm_plans
                    .iter()
                    .any(|arm| matches!(arm.resume_mode, ArmResumeMode::ImmediateResume)),
                needs_heap_continuation: plan
                    .arm_plans
                    .iter()
                    .any(|arm| matches!(arm.resume_mode, ArmResumeMode::EscapeContinuation)),
                needs_one_shot_flag: plan.frame_layout.has_one_shot_flag,
            },
            cleanup_strategy: if plan.cleanup_scopes.is_empty() {
                SimplifiedCleanupStrategy::None
            } else {
                SimplifiedCleanupStrategy::SharedGraph
            },
            suspend_sites: plan
                .suspend_sites
                .iter()
                .map(|site| SimplifiedSuspendSite::from_full_plan(plan, site))
                .collect(),
            arm_summaries: plan
                .arm_plans
                .iter()
                .map(SimplifiedArmSummary::from_full_plan_arm)
                .collect(),
            nested_handles: plan
                .nested_handles
                .iter()
                .map(Self::from_full_plan)
                .collect(),
        }
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.frame_requirements.structural_signature()
            ^ self.cleanup_strategy.structural_signature()
            ^ self.suspend_sites.len();
        for site in &self.suspend_sites {
            acc ^= site.structural_signature();
        }
        for arm in &self.arm_summaries {
            acc ^= arm.structural_signature();
        }
        for nested in &self.nested_handles {
            acc ^= nested.structural_signature();
        }
        acc
    }

    fn has_suspend_sites(&self) -> bool {
        !self.suspend_sites.is_empty()
    }

    fn arm_lowering_counts(&self) -> SimplifiedArmLoweringCounts {
        let mut counts = SimplifiedArmLoweringCounts::default();
        for arm in &self.arm_summaries {
            counts.record(arm.lowering);
        }
        counts
    }

    fn codegen_entrypoint(&self) -> SimplifiedCodegenEntrypoint {
        if !self.has_suspend_sites() {
            return SimplifiedCodegenEntrypoint::NoSuspendSites;
        }

        self.codegen_entrypoint_from_arm_mix()
    }

    fn codegen_entrypoint_from_arm_mix(&self) -> SimplifiedCodegenEntrypoint {
        let counts = self.arm_lowering_counts();
        if counts.stack_reenter > 0 && counts.heap_continuation > 1 {
            return SimplifiedCodegenEntrypoint::UnsupportedMixedMultipleEscapeWithImmediate;
        }
        if counts.stack_reenter > 1 && counts.heap_continuation > 0 {
            return SimplifiedCodegenEntrypoint::UnsupportedMixedMultipleImmediateWithEscape;
        }

        match (
            counts.flag_unwind,
            counts.stack_reenter,
            counts.heap_continuation,
        ) {
            (1, 0, 0) => SimplifiedCodegenEntrypoint::SingleNonResuming,
            (_, 0, 0) => SimplifiedCodegenEntrypoint::MultiNonResuming,
            (0, 1, 0) => SimplifiedCodegenEntrypoint::SingleImmediateResume,
            (_, 1, 0) => SimplifiedCodegenEntrypoint::ImmediateResumeWithNonResumingSiblings,
            (_, count, 0) if count > 1 => {
                SimplifiedCodegenEntrypoint::MultipleImmediateResumeTopLevel
            }
            (0, 0, 1) => SimplifiedCodegenEntrypoint::SingleEscapeContinuation,
            (_, 0, 1) => SimplifiedCodegenEntrypoint::EscapeContinuationWithNonResumingSiblings,
            (_, 0, count) if count > 1 => {
                SimplifiedCodegenEntrypoint::MultipleEscapeTopLevelDirect
            }
            (0, 1, 1) => SimplifiedCodegenEntrypoint::ImmediateResumeWithEscapeSibling,
            (_, 1, 1) => {
                SimplifiedCodegenEntrypoint::ImmediateResumeWithEscapeAndNonResumingSiblings
            }
            _ => SimplifiedCodegenEntrypoint::NoSuspendSites,
        }
    }

    #[cfg(test)]
    pub(super) fn pretty_dump(&self) -> String {
        let mut out = String::new();
        self.write_pretty_dump(0, &mut out);
        out
    }

    #[cfg(test)]
    fn write_pretty_dump(&self, indent: usize, out: &mut String) {
        let pad = " ".repeat(indent);
        out.push_str(&format!("{pad}simplification:\n"));
        out.push_str(&format!(
            "{pad}  frame payload={} cleanup={} stack-reentry={} heap-continuation={} one-shot={}\n",
            yes_no(self.frame_requirements.uses_shared_payload_transport),
            self.cleanup_strategy.label(),
            yes_no(self.frame_requirements.needs_stack_reentry),
            yes_no(self.frame_requirements.needs_heap_continuation),
            yes_no(self.frame_requirements.needs_one_shot_flag),
        ));
        out.push_str(&format!(
            "{pad}  entrypoint={}\n",
            self.codegen_entrypoint().label()
        ));

        out.push_str(&format!("{pad}  arms:\n"));
        if self.arm_summaries.is_empty() {
            out.push_str(&format!("{pad}    []\n"));
        } else {
            for arm in &self.arm_summaries {
                out.push_str(&format!(
                    "{pad}    arm{} op={} mode={} lowering={} exit={}\n",
                    arm.arm_id,
                    arm.op_fqn,
                    arm.resume_mode.label(),
                    arm.lowering.label(),
                    arm.body_exit.label(),
                ));
                out.push_str(&format!("{pad}      detach={}\n", arm.detach_policy));
            }
        }

        out.push_str(&format!("{pad}  suspend-sites:\n"));
        if self.suspend_sites.is_empty() {
            out.push_str(&format!("{pad}    []\n"));
        } else {
            for site in &self.suspend_sites {
                out.push_str(&format!(
                    "{pad}    site{} kind={} detail={} resume=s{}\n",
                    site.id,
                    site.kind.label(),
                    site.detail,
                    site.resume_target
                ));
                if site.arm_dispatch.is_empty() {
                    out.push_str(&format!("{pad}      arm-dispatch=[]\n"));
                    continue;
                }
                for arm in &site.arm_dispatch {
                    let target = arm
                        .resume_target
                        .map_or_else(|| "-".to_string(), |state| format!("s{state}"));
                    out.push_str(&format!(
                        "{pad}      arm{} op={} mode={} lowering={} exit={} target={}\n",
                        arm.arm_id,
                        arm.op_fqn,
                        arm.resume_mode.label(),
                        arm.lowering.label(),
                        arm.body_exit.label(),
                        target
                    ));
                    out.push_str(&format!("{pad}        detach={}\n", arm.detach_policy));
                }
            }
        }

        out.push_str(&format!("{pad}  nested-handles:\n"));
        if self.nested_handles.is_empty() {
            out.push_str(&format!("{pad}    []\n"));
        } else {
            for (idx, nested) in self.nested_handles.iter().enumerate() {
                out.push_str(&format!("{pad}    nested#{idx}\n"));
                nested.write_pretty_dump(indent + 6, out);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SimplifiedArmLoweringCounts {
    flag_unwind: usize,
    stack_reenter: usize,
    heap_continuation: usize,
}

impl SimplifiedArmLoweringCounts {
    fn record(&mut self, lowering: SimplifiedArmLowering) {
        match lowering {
            SimplifiedArmLowering::FlagUnwind => self.flag_unwind += 1,
            SimplifiedArmLowering::StackReenter => self.stack_reenter += 1,
            SimplifiedArmLowering::HeapContinuation => self.heap_continuation += 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SimplifiedCodegenEntrypoint {
    NoSuspendSites,
    SingleNonResuming,
    SingleImmediateResume,
    SingleEscapeContinuation,
    MultiNonResuming,
    MultipleEscapeTopLevelDirect,
    MultipleImmediateResumeTopLevel,
    ImmediateResumeWithNonResumingSiblings,
    EscapeContinuationWithNonResumingSiblings,
    ImmediateResumeWithEscapeSibling,
    ImmediateResumeWithEscapeAndNonResumingSiblings,
    UnsupportedMixedMultipleEscapeWithImmediate,
    UnsupportedMixedMultipleImmediateWithEscape,
}

impl SimplifiedCodegenEntrypoint {
    #[cfg(test)]
    fn label(self) -> &'static str {
        match self {
            SimplifiedCodegenEntrypoint::NoSuspendSites => "no-suspend-sites",
            SimplifiedCodegenEntrypoint::SingleNonResuming => "single-nonresuming",
            SimplifiedCodegenEntrypoint::SingleImmediateResume => "single-immediate-resume",
            SimplifiedCodegenEntrypoint::SingleEscapeContinuation => "single-escape-continuation",
            SimplifiedCodegenEntrypoint::MultiNonResuming => "multi-nonresuming",
            SimplifiedCodegenEntrypoint::MultipleEscapeTopLevelDirect => {
                "multiple-escape-top-level-direct"
            }
            SimplifiedCodegenEntrypoint::MultipleImmediateResumeTopLevel => {
                "multiple-immediate-top-level"
            }
            SimplifiedCodegenEntrypoint::ImmediateResumeWithNonResumingSiblings => {
                "immediate-with-nonresuming"
            }
            SimplifiedCodegenEntrypoint::EscapeContinuationWithNonResumingSiblings => {
                "escape-with-nonresuming"
            }
            SimplifiedCodegenEntrypoint::ImmediateResumeWithEscapeSibling => {
                "immediate-with-escape"
            }
            SimplifiedCodegenEntrypoint::ImmediateResumeWithEscapeAndNonResumingSiblings => {
                "immediate-with-escape-and-nonresuming"
            }
            SimplifiedCodegenEntrypoint::UnsupportedMixedMultipleEscapeWithImmediate => {
                "unsupported-mixed-multiple-escape-with-immediate"
            }
            SimplifiedCodegenEntrypoint::UnsupportedMixedMultipleImmediateWithEscape => {
                "unsupported-mixed-multiple-immediate-with-escape"
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SimplifiedFrameRequirements {
    uses_shared_payload_transport: bool,
    needs_stack_reentry: bool,
    needs_heap_continuation: bool,
    needs_one_shot_flag: bool,
}

impl SimplifiedFrameRequirements {
    fn structural_signature(&self) -> usize {
        usize::from(self.uses_shared_payload_transport)
            ^ (usize::from(self.needs_stack_reentry) << 1)
            ^ (usize::from(self.needs_heap_continuation) << 2)
            ^ (usize::from(self.needs_one_shot_flag) << 3)
    }
}

#[derive(Debug, Clone, Copy)]
enum SimplifiedCleanupStrategy {
    None,
    SharedGraph,
}

impl SimplifiedCleanupStrategy {
    fn structural_signature(self) -> usize {
        match self {
            SimplifiedCleanupStrategy::None => 1,
            SimplifiedCleanupStrategy::SharedGraph => 2,
        }
    }

    #[cfg(test)]
    fn label(self) -> &'static str {
        match self {
            SimplifiedCleanupStrategy::None => "none",
            SimplifiedCleanupStrategy::SharedGraph => "shared-graph",
        }
    }
}

#[derive(Debug, Clone)]
struct SimplifiedSuspendSite {
    id: SuspendSiteId,
    kind: SimplifiedSuspendSiteKind,
    detail: String,
    resume_target: PlanStateId,
    arm_dispatch: Vec<SimplifiedArmDispatch>,
}

impl SimplifiedSuspendSite {
    fn from_full_plan(plan: &HandleStateMachinePlan, site: &SuspendSitePlan) -> Self {
        Self {
            id: site.id,
            kind: SimplifiedSuspendSiteKind::from_site_kind(&site.kind),
            detail: SimplifiedSuspendSiteKind::detail_for_site(&site.kind),
            resume_target: site.resume_target,
            arm_dispatch: site
                .matching_arms
                .iter()
                .map(|arm_id| {
                    let arm = plan
                        .arm_plans
                        .iter()
                        .find(|arm| arm.id == *arm_id)
                        .expect("matching arm should exist");
                    SimplifiedArmDispatch::from_full_plan_arm(site.resume_target, arm)
                })
                .collect(),
        }
    }

    fn structural_signature(&self) -> usize {
        let mut acc = self.id as usize
            ^ self.kind.structural_signature()
            ^ self.detail.len()
            ^ self.resume_target as usize;
        for arm in &self.arm_dispatch {
            acc ^= arm.structural_signature();
        }
        acc
    }
}

#[derive(Debug, Clone, Copy)]
enum SimplifiedSuspendSiteKind {
    DirectPerform,
    IndirectCallMaySuspend,
    CallStateMachineCallee,
}

impl SimplifiedSuspendSiteKind {
    fn from_site_kind(kind: &SuspendSiteKind) -> Self {
        match kind {
            SuspendSiteKind::DirectPerform { .. } => SimplifiedSuspendSiteKind::DirectPerform,
            SuspendSiteKind::IndirectCallMaySuspend { .. } => {
                SimplifiedSuspendSiteKind::IndirectCallMaySuspend
            }
            SuspendSiteKind::CallStateMachineCallee { .. } => {
                SimplifiedSuspendSiteKind::CallStateMachineCallee
            }
        }
    }

    fn detail_for_site(kind: &SuspendSiteKind) -> String {
        match kind {
            SuspendSiteKind::DirectPerform { op_fqn }
            | SuspendSiteKind::IndirectCallMaySuspend { callee: op_fqn }
            | SuspendSiteKind::CallStateMachineCallee { callee: op_fqn } => op_fqn.clone(),
        }
    }

    fn structural_signature(self) -> usize {
        match self {
            SimplifiedSuspendSiteKind::DirectPerform => 1,
            SimplifiedSuspendSiteKind::IndirectCallMaySuspend => 2,
            SimplifiedSuspendSiteKind::CallStateMachineCallee => 3,
        }
    }

    #[cfg(test)]
    fn label(self) -> &'static str {
        match self {
            SimplifiedSuspendSiteKind::DirectPerform => "direct-perform",
            SimplifiedSuspendSiteKind::IndirectCallMaySuspend => "indirect-call-may-suspend",
            SimplifiedSuspendSiteKind::CallStateMachineCallee => "call-state-machine-callee",
        }
    }
}

#[derive(Debug, Clone)]
struct SimplifiedArmDispatch {
    arm_id: ArmPlanId,
    op_fqn: String,
    resume_mode: ArmResumeMode,
    lowering: SimplifiedArmLowering,
    body_exit: ArmBodyExit,
    resume_target: Option<PlanStateId>,
    detach_policy: String,
}

impl SimplifiedArmDispatch {
    fn from_full_plan_arm(resume_target: PlanStateId, arm: &ArmPlan) -> Self {
        let lowering = match arm.resume_mode {
            ArmResumeMode::NeverResume => SimplifiedArmLowering::FlagUnwind,
            ArmResumeMode::ImmediateResume => SimplifiedArmLowering::StackReenter,
            ArmResumeMode::EscapeContinuation => SimplifiedArmLowering::HeapContinuation,
        };

        let resume_target = match lowering {
            SimplifiedArmLowering::FlagUnwind => None,
            SimplifiedArmLowering::StackReenter | SimplifiedArmLowering::HeapContinuation => {
                Some(resume_target)
            }
        };

        Self {
            arm_id: arm.id,
            op_fqn: arm.op_fqn.clone(),
            resume_mode: arm.resume_mode,
            lowering,
            body_exit: arm.body_exit,
            resume_target,
            detach_policy: arm.detach_policy.clone(),
        }
    }

    fn structural_signature(&self) -> usize {
        self.arm_id as usize
            ^ self.op_fqn.len()
            ^ self.resume_mode.structural_signature()
            ^ self.lowering.structural_signature()
            ^ self.body_exit.structural_signature()
            ^ ((self.resume_target.unwrap_or(0) as usize) << 1)
            ^ self.detach_policy.len()
    }
}

#[derive(Debug, Clone)]
struct SimplifiedArmSummary {
    arm_id: ArmPlanId,
    op_fqn: String,
    resume_mode: ArmResumeMode,
    lowering: SimplifiedArmLowering,
    body_exit: ArmBodyExit,
    detach_policy: String,
}

impl SimplifiedArmSummary {
    fn from_full_plan_arm(arm: &ArmPlan) -> Self {
        Self {
            arm_id: arm.id,
            op_fqn: arm.op_fqn.clone(),
            resume_mode: arm.resume_mode,
            lowering: match arm.resume_mode {
                ArmResumeMode::NeverResume => SimplifiedArmLowering::FlagUnwind,
                ArmResumeMode::ImmediateResume => SimplifiedArmLowering::StackReenter,
                ArmResumeMode::EscapeContinuation => SimplifiedArmLowering::HeapContinuation,
            },
            body_exit: arm.body_exit,
            detach_policy: arm.detach_policy.clone(),
        }
    }

    fn structural_signature(&self) -> usize {
        self.arm_id as usize
            ^ self.op_fqn.len()
            ^ self.resume_mode.structural_signature()
            ^ self.lowering.structural_signature()
            ^ self.body_exit.structural_signature()
            ^ self.detach_policy.len()
    }
}

#[derive(Debug, Clone, Copy)]
enum SimplifiedArmLowering {
    FlagUnwind,
    StackReenter,
    HeapContinuation,
}

impl SimplifiedArmLowering {
    fn structural_signature(self) -> usize {
        match self {
            SimplifiedArmLowering::FlagUnwind => 1,
            SimplifiedArmLowering::StackReenter => 2,
            SimplifiedArmLowering::HeapContinuation => 3,
        }
    }

    #[cfg(test)]
    fn label(self) -> &'static str {
        match self {
            SimplifiedArmLowering::FlagUnwind => "flag-unwind",
            SimplifiedArmLowering::StackReenter => "stack-reenter",
            SimplifiedArmLowering::HeapContinuation => "heap-continuation",
        }
    }
}

impl HandleStateMachinePlan {
    fn build_mode_specific_simplification(&self) -> HandleModeSpecificSimplification {
        HandleModeSpecificSimplification::from_full_plan(self)
    }
}

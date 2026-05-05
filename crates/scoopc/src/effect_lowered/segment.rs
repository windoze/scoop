use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::effect_facts::{BodyEffectFacts, NestedHandleClassification, SiteEffectFacts};
use crate::mir::{
    BasicBlockId, Body, LocalId, Operand, Rvalue, SiteId, StatementKind, Terminator,
    TerminatorKind, UnwindAction,
};
use crate::ty::TypeId;

use super::EffectLoweringError;
use super::ir::{
    BoundaryId, BoundarySiteKind, LateLoweredBoundary, LateLoweredBoundaryMap,
    LateLoweredBoundarySource, LateLoweredCompletionPayloadSource, LateLoweredOperandSource,
    LateLoweredResumeState, LateLoweredResumeStateMap, LateLoweredState, LateLoweredStateGraph,
    LateLoweredStateRole, LateLoweredStateSlice, LateLoweredStateTerminator, StateId,
};

/// P5-T03 产出的 whole-function segmentation skeleton。
pub(crate) struct LateLoweredSegmentation {
    pub(crate) state_graph: LateLoweredStateGraph,
    pub(crate) boundary_map: LateLoweredBoundaryMap,
    pub(crate) resume_state_map: LateLoweredResumeStateMap,
}

pub(crate) fn build_callable_segmentation(
    root_fqn: &str,
    body: &Body,
    body_facts: &BodyEffectFacts,
    complete_ty: TypeId,
) -> Result<LateLoweredSegmentation, EffectLoweringError> {
    SegmentationBuilder::new(root_fqn, body, body_facts, complete_ty)?.build()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct StateCursor {
    block: BasicBlockId,
    statement_index: u32,
}

impl StateCursor {
    const fn block_start(block: BasicBlockId) -> Self {
        Self {
            block,
            statement_index: 0,
        }
    }

    const fn after_statement(block: BasicBlockId, statement_index: u32) -> Self {
        Self {
            block,
            statement_index: statement_index.saturating_add(1),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum BoundaryAnchor {
    Statement {
        block: BasicBlockId,
        statement_index: u32,
    },
    Terminator {
        block: BasicBlockId,
    },
}

impl BoundaryAnchor {
    fn describe(self) -> String {
        match self {
            Self::Statement {
                block,
                statement_index,
            } => format!("bb{}:stmt{}", block.as_u32(), statement_index),
            Self::Terminator { block } => format!("bb{}:term", block.as_u32()),
        }
    }
}

#[derive(Debug, Clone)]
struct SelectedBoundary {
    sources: Vec<LateLoweredBoundarySource>,
    resume_cursor: StateCursor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AnchorBinding {
    owner_state: StateId,
    resume_state: StateId,
}

#[derive(Debug, Clone)]
struct StateBlueprint {
    source_slices: Vec<LateLoweredStateSlice>,
    terminator: StateBlueprintTerminator,
}

#[derive(Debug, Clone)]
enum StateBlueprintTerminator {
    Suspend {
        anchor: BoundaryAnchor,
        resume_state: StateId,
        cleanup_target: Option<StateId>,
    },
    Goto {
        target: StateId,
    },
    Branch {
        cond_local: LocalId,
        then_state: StateId,
        else_state: StateId,
    },
    Return {
        payload_source: LateLoweredCompletionPayloadSource,
    },
    HandleDispatch {
        site_id: SiteId,
        body_state: StateId,
        arm_states: Vec<StateId>,
        finally_state: Option<StateId>,
        exit_state: StateId,
        boundary_anchor: Option<BoundaryAnchor>,
    },
    ResumeUnwind,
    Unreachable,
}

struct SegmentationBuilder<'a> {
    root_fqn: &'a str,
    body: &'a Body,
    complete_ty: TypeId,
    selected_boundaries: BTreeMap<BoundaryAnchor, SelectedBoundary>,
    cursor_ids: BTreeMap<StateCursor, StateId>,
    pending: VecDeque<StateCursor>,
    built: BTreeMap<StateCursor, StateBlueprint>,
    resume_cursors: BTreeSet<StateCursor>,
    anchor_bindings: BTreeMap<BoundaryAnchor, AnchorBinding>,
    entry_state: StateId,
    complete_state: StateId,
    next_state_raw: u32,
}

impl<'a> SegmentationBuilder<'a> {
    fn new(
        root_fqn: &'a str,
        body: &'a Body,
        body_facts: &BodyEffectFacts,
        complete_ty: TypeId,
    ) -> Result<Self, EffectLoweringError> {
        let selected_boundaries = collect_selected_boundaries(root_fqn, body, body_facts)?;
        let entry_cursor = StateCursor::block_start(body.start);
        let entry_state = StateId::new(0);
        let complete_state = StateId::new(1);
        let mut cursor_ids = BTreeMap::new();
        cursor_ids.insert(entry_cursor, entry_state);
        let mut pending = VecDeque::new();
        pending.push_back(entry_cursor);
        Ok(Self {
            root_fqn,
            body,
            complete_ty,
            selected_boundaries,
            cursor_ids,
            pending,
            built: BTreeMap::new(),
            resume_cursors: BTreeSet::new(),
            anchor_bindings: BTreeMap::new(),
            entry_state,
            complete_state,
            next_state_raw: 2,
        })
    }

    fn build(mut self) -> Result<LateLoweredSegmentation, EffectLoweringError> {
        while let Some(cursor) = self.pending.pop_front() {
            if self.built.contains_key(&cursor) {
                continue;
            }
            let blueprint = self.build_state(cursor)?;
            self.built.insert(cursor, blueprint);
        }

        for anchor in self.selected_boundaries.keys() {
            if !self.anchor_bindings.contains_key(anchor) {
                return Err(EffectLoweringError::UnboundBoundary {
                    root_fqn: self.root_fqn.to_string(),
                    description: anchor.describe(),
                });
            }
        }

        let id_to_cursor = self
            .cursor_ids
            .iter()
            .map(|(cursor, state_id)| (*state_id, *cursor))
            .collect::<BTreeMap<_, _>>();

        let mut boundary_entries = Vec::new();
        let mut resume_entries = Vec::new();
        let mut boundary_ids_by_anchor = BTreeMap::<BoundaryAnchor, Vec<BoundaryId>>::new();
        let mut next_boundary_raw = 0u32;
        for (anchor, boundary) in &self.selected_boundaries {
            let binding = self
                .anchor_bindings
                .get(anchor)
                .expect("selected boundary should already bind owner/resume states");
            for source in &boundary.sources {
                let boundary_id = BoundaryId::new(next_boundary_raw);
                next_boundary_raw += 1;
                boundary_entries.push(LateLoweredBoundary::new(
                    boundary_id,
                    *source,
                    binding.owner_state,
                    binding.resume_state,
                ));
                resume_entries.push(LateLoweredResumeState::new(
                    boundary_id,
                    binding.resume_state,
                ));
                boundary_ids_by_anchor
                    .entry(*anchor)
                    .or_default()
                    .push(boundary_id);
            }
        }

        let mut cleanup_state = None;
        let mut states = Vec::with_capacity(self.next_state_raw as usize);
        for raw in 0..self.next_state_raw {
            let state_id = StateId::new(raw);
            if state_id == self.complete_state {
                states.push(LateLoweredState::new(
                    state_id,
                    LateLoweredStateRole::Complete,
                    Vec::new(),
                    LateLoweredStateTerminator::Unreachable,
                ));
                continue;
            }

            let cursor = *id_to_cursor
                .get(&state_id)
                .expect("every non-synthetic state id should map back to a cursor");
            let blueprint = self
                .built
                .get(&cursor)
                .expect("every discovered cursor should publish a state blueprint");
            let role = self.role_for(cursor, state_id);
            if cleanup_state.is_none() && role == LateLoweredStateRole::Cleanup {
                cleanup_state = Some(state_id);
            }
            let terminator = finalize_blueprint_terminator(
                &blueprint.terminator,
                &boundary_ids_by_anchor,
                self.complete_state,
            );
            states.push(LateLoweredState::new(
                state_id,
                role,
                blueprint.source_slices.clone(),
                terminator,
            ));
        }

        Ok(LateLoweredSegmentation {
            state_graph: LateLoweredStateGraph::new(
                self.entry_state,
                self.complete_state,
                cleanup_state,
                None,
                states,
            ),
            boundary_map: LateLoweredBoundaryMap::new(boundary_entries),
            resume_state_map: LateLoweredResumeStateMap::new(resume_entries),
        })
    }

    fn role_for(&self, cursor: StateCursor, state_id: StateId) -> LateLoweredStateRole {
        if state_id == self.entry_state {
            return LateLoweredStateRole::Entry;
        }
        if self.body.blocks[cursor.block.as_u32() as usize].is_cleanup {
            return LateLoweredStateRole::Cleanup;
        }
        if self.resume_cursors.contains(&cursor) {
            return LateLoweredStateRole::Resume;
        }
        LateLoweredStateRole::Segment
    }

    fn build_state(&mut self, cursor: StateCursor) -> Result<StateBlueprint, EffectLoweringError> {
        let block = &self.body.blocks[cursor.block.as_u32() as usize];
        let start = cursor.statement_index as usize;
        let statement_len = block.stmts.len();
        debug_assert!(
            start <= statement_len,
            "state cursor should not point past block end"
        );

        for statement_index in start..statement_len {
            let anchor = BoundaryAnchor::Statement {
                block: cursor.block,
                statement_index: statement_index as u32,
            };
            let Some(boundary) = self.selected_boundaries.get(&anchor).cloned() else {
                continue;
            };
            let owner_state = self.state_id(cursor);
            let resume_state = self.ensure_state(boundary.resume_cursor, true);
            self.anchor_bindings.insert(
                anchor,
                AnchorBinding {
                    owner_state,
                    resume_state,
                },
            );
            return Ok(StateBlueprint {
                source_slices: vec![LateLoweredStateSlice::new(
                    cursor.block,
                    cursor.statement_index,
                    statement_index as u32 + 1,
                    false,
                )],
                terminator: StateBlueprintTerminator::Suspend {
                    anchor,
                    resume_state,
                    cleanup_target: None,
                },
            });
        }

        let anchor = BoundaryAnchor::Terminator {
            block: cursor.block,
        };
        let selected_boundary = self.selected_boundaries.get(&anchor).cloned();
        if let Some(boundary) = selected_boundary.as_ref() {
            let owner_state = self.state_id(cursor);
            let resume_state = self.ensure_state(boundary.resume_cursor, true);
            self.anchor_bindings.insert(
                anchor,
                AnchorBinding {
                    owner_state,
                    resume_state,
                },
            );
        }
        let boundary_anchor = selected_boundary.as_ref().map(|_| anchor);

        Ok(StateBlueprint {
            source_slices: vec![LateLoweredStateSlice::new(
                cursor.block,
                cursor.statement_index,
                statement_len as u32,
                true,
            )],
            terminator: self.build_terminator_blueprint(&block.terminator, boundary_anchor)?,
        })
    }

    fn build_terminator_blueprint(
        &mut self,
        terminator: &Terminator,
        boundary_anchor: Option<BoundaryAnchor>,
    ) -> Result<StateBlueprintTerminator, EffectLoweringError> {
        match &terminator.kind {
            TerminatorKind::Return { value } => Ok(StateBlueprintTerminator::Return {
                payload_source: self.completion_payload_source(terminator, value)?,
            }),
            TerminatorKind::Goto { target } => Ok(StateBlueprintTerminator::Goto {
                target: self.ensure_state(StateCursor::block_start(*target), false),
            }),
            TerminatorKind::CondBr {
                cond,
                then_target,
                else_target,
            } => Ok(StateBlueprintTerminator::Branch {
                cond_local: operand_local(cond)
                    .expect("direct-style CondBr should lower condition into a local"),
                then_state: self.ensure_state(StateCursor::block_start(*then_target), false),
                else_state: self.ensure_state(StateCursor::block_start(*else_target), false),
            }),
            TerminatorKind::Perform { resume_target, .. } if boundary_anchor.is_some() => {
                Ok(StateBlueprintTerminator::Suspend {
                    anchor: boundary_anchor.expect("perform boundary anchor should exist"),
                    resume_state: self.ensure_state(StateCursor::block_start(*resume_target), true),
                    cleanup_target: cleanup_state_from_unwind(self, &terminator.unwind),
                })
            }
            TerminatorKind::Handle {
                site_id,
                body_target,
                arm_targets,
                finally_target,
                exit_target,
                ..
            } => Ok(StateBlueprintTerminator::HandleDispatch {
                site_id: *site_id,
                body_state: self.ensure_state(StateCursor::block_start(*body_target), false),
                arm_states: arm_targets
                    .iter()
                    .map(|target| self.ensure_state(StateCursor::block_start(*target), false))
                    .collect(),
                finally_state: finally_target
                    .map(|target| self.ensure_state(StateCursor::block_start(target), false)),
                exit_state: self.ensure_state(
                    StateCursor::block_start(*exit_target),
                    boundary_anchor.is_some(),
                ),
                boundary_anchor,
            }),
            TerminatorKind::ResumeUnwind => Ok(StateBlueprintTerminator::ResumeUnwind),
            TerminatorKind::Unreachable | TerminatorKind::Todo(_) => {
                Ok(StateBlueprintTerminator::Unreachable)
            }
            TerminatorKind::Perform { resume_target, .. } => Ok(StateBlueprintTerminator::Goto {
                target: self.ensure_state(StateCursor::block_start(*resume_target), true),
            }),
        }
    }

    fn completion_payload_source(
        &self,
        terminator: &Terminator,
        value: &Option<Operand>,
    ) -> Result<LateLoweredCompletionPayloadSource, EffectLoweringError> {
        let Some(value) = value else {
            return Ok(LateLoweredCompletionPayloadSource::unit(self.complete_ty));
        };
        match value {
            Operand::Local(local) => {
                let local_ty = self
                    .body
                    .locals
                    .get(local.as_u32() as usize)
                    .map(|decl| decl.ty)
                    .ok_or_else(|| EffectLoweringError::InvalidCompletionPayloadContract {
                        root_fqn: self.root_fqn.to_string(),
                        detail: format!("return payload 引用了不存在的 local{}", local.as_u32()),
                    })?;
                if local_ty != self.complete_ty {
                    return Err(EffectLoweringError::InvalidCompletionPayloadContract {
                        root_fqn: self.root_fqn.to_string(),
                        detail: format!(
                            "return payload local{} 的类型为 t{}，但 Step complete_ty 为 t{}",
                            local.as_u32(),
                            local_ty.as_u32(),
                            self.complete_ty.as_u32()
                        ),
                    });
                }
                Ok(LateLoweredCompletionPayloadSource::operand(
                    LateLoweredOperandSource::new_local(
                        *local,
                        self.complete_ty,
                        Some(terminator.span),
                    ),
                ))
            }
            Operand::Const(value) => Ok(LateLoweredCompletionPayloadSource::operand(
                LateLoweredOperandSource::new_const(
                    value.clone(),
                    self.complete_ty,
                    Some(terminator.span),
                ),
            )),
        }
    }

    fn ensure_state(&mut self, cursor: StateCursor, is_resume: bool) -> StateId {
        if is_resume {
            self.resume_cursors.insert(cursor);
        }
        if let Some(state_id) = self.cursor_ids.get(&cursor) {
            return *state_id;
        }
        let state_id = StateId::new(self.next_state_raw);
        self.next_state_raw += 1;
        self.cursor_ids.insert(cursor, state_id);
        self.pending.push_back(cursor);
        state_id
    }

    fn state_id(&self, cursor: StateCursor) -> StateId {
        *self
            .cursor_ids
            .get(&cursor)
            .expect("every built state should already have a stable state id")
    }
}

fn finalize_blueprint_terminator(
    blueprint: &StateBlueprintTerminator,
    boundary_ids_by_anchor: &BTreeMap<BoundaryAnchor, Vec<BoundaryId>>,
    complete_state: StateId,
) -> LateLoweredStateTerminator {
    match blueprint {
        StateBlueprintTerminator::Suspend {
            anchor,
            resume_state,
            cleanup_target,
        } => LateLoweredStateTerminator::Suspend {
            boundary_ids: boundary_ids_by_anchor
                .get(anchor)
                .cloned()
                .unwrap_or_default(),
            resume_state: *resume_state,
            local_runtime_error_states: Vec::new(),
            cleanup_state: *cleanup_target,
            drop_state: None,
        },
        StateBlueprintTerminator::Goto { target } => {
            LateLoweredStateTerminator::Goto { target: *target }
        }
        StateBlueprintTerminator::Branch {
            cond_local,
            then_state,
            else_state,
        } => LateLoweredStateTerminator::Branch {
            cond_local: *cond_local,
            then_state: *then_state,
            else_state: *else_state,
        },
        StateBlueprintTerminator::Return { payload_source } => LateLoweredStateTerminator::Return {
            payload_source: payload_source.clone(),
            complete_state,
        },
        StateBlueprintTerminator::HandleDispatch {
            site_id,
            body_state,
            arm_states,
            finally_state,
            exit_state,
            boundary_anchor,
        } => {
            let body_complete_target = finally_state.unwrap_or(*exit_state);
            let arm_complete_target = finally_state.unwrap_or(*exit_state);
            let contract = crate::effect_lowered::ir::LateLoweredHandleDispatchContract::skeleton(
                body_complete_target,
                arm_complete_target,
                finally_state.map(|_| *exit_state),
                None,
            );
            LateLoweredStateTerminator::HandleDispatch {
                site_id: *site_id,
                body_state: *body_state,
                arm_states: arm_states.clone(),
                finally_state: *finally_state,
                exit_state: *exit_state,
                contract,
                boundary_ids: boundary_anchor
                    .and_then(|anchor| boundary_ids_by_anchor.get(&anchor).cloned())
                    .unwrap_or_default(),
                drop_state: None,
            }
        }
        StateBlueprintTerminator::ResumeUnwind => LateLoweredStateTerminator::ResumeUnwind,
        StateBlueprintTerminator::Unreachable => LateLoweredStateTerminator::Unreachable,
    }
}

fn cleanup_state_from_unwind(
    builder: &mut SegmentationBuilder<'_>,
    unwind: &UnwindAction,
) -> Option<StateId> {
    match unwind {
        UnwindAction::Cleanup { target } => {
            Some(builder.ensure_state(StateCursor::block_start(*target), false))
        }
        UnwindAction::NoUnwind | UnwindAction::Propagate | UnwindAction::Todo(_) => None,
    }
}

fn operand_local(operand: &Operand) -> Option<LocalId> {
    match operand {
        Operand::Local(local) => Some(*local),
        Operand::Const(_) => None,
    }
}

fn collect_selected_boundaries(
    root_fqn: &str,
    body: &Body,
    body_facts: &BodyEffectFacts,
) -> Result<BTreeMap<BoundaryAnchor, SelectedBoundary>, EffectLoweringError> {
    let mut selected = BTreeMap::new();
    let nested_handle_sites = collect_nested_handle_sites(body);

    for (block_index, block) in body.blocks.iter().enumerate() {
        let block_id = BasicBlockId::from_raw(block_index as u32);
        for (statement_index, stmt) in block.stmts.iter().enumerate() {
            let StatementKind::Assign { value, .. } = &stmt.kind else {
                continue;
            };
            let (site_id, is_class_ctor) = match value {
                Rvalue::Call { site_id, .. } => (*site_id, false),
                Rvalue::ClassCtor { site_id, .. } => (*site_id, true),
                _ => continue,
            };
            let site =
                body_facts
                    .site(site_id)
                    .ok_or_else(|| EffectLoweringError::MissingSiteFacts {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                    })?;
            let anchor = BoundaryAnchor::Statement {
                block: block_id,
                statement_index: statement_index as u32,
            };
            let resume_cursor = StateCursor::after_statement(block_id, statement_index as u32);
            match site {
                SiteEffectFacts::ClassCtor(class_ctor_facts) if is_class_ctor => {
                    if class_ctor_facts.emitted_cases().is_empty() {
                        continue;
                    }
                    push_selected_boundary(
                        &mut selected,
                        anchor,
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::ClassCtor,
                        },
                        resume_cursor,
                    );
                }
                SiteEffectFacts::Call(call_facts) => {
                    if is_class_ctor {
                        return Err(EffectLoweringError::UnexpectedSiteFactsKind {
                            root_fqn: root_fqn.to_string(),
                            site_id: site_id.as_u32(),
                            expected: "ClassCtor",
                            actual: site_facts_kind(site),
                        });
                    }
                    if call_facts.resolved_cases().is_empty() {
                        continue;
                    }
                    push_selected_boundary(
                        &mut selected,
                        anchor,
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::Call,
                        },
                        resume_cursor,
                    );
                }
                SiteEffectFacts::Resume(_) => {
                    push_selected_boundary(
                        &mut selected,
                        anchor,
                        LateLoweredBoundarySource::Site {
                            site_id,
                            kind: BoundarySiteKind::Resume,
                        },
                        resume_cursor,
                    );
                    push_selected_boundary(
                        &mut selected,
                        anchor,
                        LateLoweredBoundarySource::RuntimeError {
                            origin_site: site_id,
                        },
                        resume_cursor,
                    );
                }
                other => {
                    return Err(EffectLoweringError::UnexpectedSiteFactsKind {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        expected: if is_class_ctor {
                            "ClassCtor"
                        } else {
                            "Call or Resume"
                        },
                        actual: site_facts_kind(other),
                    });
                }
            }
        }

        match &block.terminator.kind {
            TerminatorKind::Perform {
                site_id,
                resume_target,
                ..
            } => {
                let site = body_facts.site(*site_id).ok_or_else(|| {
                    EffectLoweringError::MissingSiteFacts {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                    }
                })?;
                let SiteEffectFacts::Perform(_) = site else {
                    return Err(EffectLoweringError::UnexpectedSiteFactsKind {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        expected: "Perform",
                        actual: site_facts_kind(site),
                    });
                };
                push_selected_boundary(
                    &mut selected,
                    BoundaryAnchor::Terminator { block: block_id },
                    LateLoweredBoundarySource::Site {
                        site_id: *site_id,
                        kind: BoundarySiteKind::Perform,
                    },
                    StateCursor::block_start(*resume_target),
                );
            }
            TerminatorKind::Handle {
                site_id,
                exit_target,
                ..
            } => {
                let site = body_facts.site(*site_id).ok_or_else(|| {
                    EffectLoweringError::MissingSiteFacts {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                    }
                })?;
                let SiteEffectFacts::Handle(handle_facts) = site else {
                    return Err(EffectLoweringError::UnexpectedSiteFactsKind {
                        root_fqn: root_fqn.to_string(),
                        site_id: site_id.as_u32(),
                        expected: "Handle",
                        actual: site_facts_kind(site),
                    });
                };
                if nested_handle_sites.contains(site_id)
                    && handle_facts.nested_handle_classification()
                        == NestedHandleClassification::MaySuspendOutward
                {
                    push_selected_boundary(
                        &mut selected,
                        BoundaryAnchor::Terminator { block: block_id },
                        LateLoweredBoundarySource::Site {
                            site_id: *site_id,
                            kind: BoundarySiteKind::Handle,
                        },
                        StateCursor::block_start(*exit_target),
                    );
                }
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }
    }

    Ok(selected)
}

fn collect_nested_handle_sites(body: &Body) -> BTreeSet<SiteId> {
    let mut nested = BTreeSet::new();
    for block in &body.blocks {
        let TerminatorKind::Handle {
            body_target,
            arm_targets,
            finally_target,
            exit_target,
            ..
        } = &block.terminator.kind
        else {
            continue;
        };

        let mut stops = BTreeSet::from([*exit_target]);
        if let Some(finally_target) = finally_target {
            stops.insert(*finally_target);
        }
        let mut visited = BTreeSet::new();
        collect_region_blocks(body, *body_target, &stops, &mut visited);
        for arm_target in arm_targets {
            collect_region_blocks(body, *arm_target, &stops, &mut visited);
        }
        if let Some(finally_target) = finally_target {
            let finally_stops = BTreeSet::from([*exit_target]);
            collect_region_blocks(body, *finally_target, &finally_stops, &mut visited);
        }

        for block_id in visited {
            if let TerminatorKind::Handle { site_id, .. } =
                body.blocks[block_id.as_u32() as usize].terminator.kind
            {
                nested.insert(site_id);
            }
        }
    }
    nested
}

fn collect_region_blocks(
    body: &Body,
    entry: BasicBlockId,
    stops: &BTreeSet<BasicBlockId>,
    visited: &mut BTreeSet<BasicBlockId>,
) {
    if stops.contains(&entry) || !visited.insert(entry) {
        return;
    }

    body.blocks[entry.as_u32() as usize]
        .terminator
        .for_each_successor(|target| collect_region_blocks(body, target, stops, visited));
}

fn push_selected_boundary(
    selected: &mut BTreeMap<BoundaryAnchor, SelectedBoundary>,
    anchor: BoundaryAnchor,
    source: LateLoweredBoundarySource,
    resume_cursor: StateCursor,
) {
    match selected.get_mut(&anchor) {
        Some(existing) => {
            debug_assert_eq!(existing.resume_cursor, resume_cursor);
            existing.sources.push(source);
        }
        None => {
            selected.insert(
                anchor,
                SelectedBoundary {
                    sources: vec![source],
                    resume_cursor,
                },
            );
        }
    }
}

fn site_facts_kind(site: &SiteEffectFacts) -> &'static str {
    match site {
        SiteEffectFacts::Call(_) => "Call",
        SiteEffectFacts::ClassCtor(_) => "ClassCtor",
        SiteEffectFacts::Perform(_) => "Perform",
        SiteEffectFacts::Resume(_) => "Resume",
        SiteEffectFacts::Handle(_) => "Handle",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::build_callable_segmentation;
    use crate::effect_facts::CallableAbiKind;
    use crate::effect_lowered::ir::{
        BoundarySiteKind, LateLoweredBoundarySource, LateLoweredStateRole,
    };
    use crate::effect_refactor_pipeline::load_effect_lowered_stage_output_for_dump;
    use crate::session::{EffectPipelineMode, Session, SessionOptions};
    use crate::source::SourceFile;

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn load_fixture(phase: &str, name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures")
            .join(phase)
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    fn load_output(
        source: &SourceFile,
    ) -> crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput {
        let session = refactor_session();
        load_effect_lowered_stage_output_for_dump(&session, source)
            .expect("fixture 应可通过 refactor late-lowering stage")
    }

    fn callable<'a>(
        output: &'a crate::effect_refactor_pipeline::RefactorEffectLoweredStageOutput,
        fqn: &str,
    ) -> &'a crate::effect_lowered::LateLoweredCallable {
        output
            .program()
            .callable(fqn)
            .unwrap_or_else(|| panic!("late-lowered program 应发布 {fqn}"))
    }

    fn site_boundaries(
        callable: &crate::effect_lowered::LateLoweredCallable,
        kind: BoundarySiteKind,
    ) -> Vec<&crate::effect_lowered::ir::LateLoweredBoundary> {
        callable
            .boundary_map()
            .entries()
            .iter()
            .filter(|boundary| {
                matches!(
                    boundary.source(),
                    LateLoweredBoundarySource::Site { kind: source_kind, .. } if source_kind == kind
                )
            })
            .collect()
    }

    #[test]
    fn refactor_late_boundary_selection_marks_call_resume_runtime_error_perform_and_outward_handle_boundaries()
     {
        let call_output = load_output(&load_fixture(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
        ));
        let call_value = callable(&call_output, "sample.callValue");
        let call_boundaries = site_boundaries(call_value, BoundarySiteKind::Call);
        assert_eq!(call_boundaries.len(), 1);

        let resume_output = load_output(&load_fixture(
            "effect_facts",
            "dispatch_and_resume_call.scoop",
        ));
        let resume_once = callable(&resume_output, "fixtures.mir.resumeOnce");
        let resume_boundaries = site_boundaries(resume_once, BoundarySiteKind::Resume);
        assert_eq!(resume_boundaries.len(), 1);
        let runtime_error_boundaries = resume_once
            .boundary_map()
            .entries()
            .iter()
            .filter(|boundary| {
                matches!(
                    boundary.source(),
                    LateLoweredBoundarySource::RuntimeError { .. }
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(runtime_error_boundaries.len(), 1);
        assert_eq!(
            resume_boundaries[0].owner_state(),
            runtime_error_boundaries[0].owner_state(),
            "resume site 与其 runtime error boundary 应共享同一个 owner state"
        );
        assert_eq!(
            resume_boundaries[0].resume_state(),
            runtime_error_boundaries[0].resume_state(),
            "resume site 与其 runtime error boundary 应共享同一个 resume state"
        );

        let perform_output = load_output(&load_fixture("effect_facts", "handle_perform.scoop"));
        let handled_main = callable(&perform_output, "a.main");
        assert_eq!(
            site_boundaries(handled_main, BoundarySiteKind::Perform).len(),
            1
        );

        let nested_output = load_output(&load_fixture(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
        ));
        let outward = callable(&nested_output, "sample.nested_may_suspend_outward");
        assert_eq!(site_boundaries(outward, BoundarySiteKind::Handle).len(), 1);
    }

    #[test]
    fn refactor_late_boundary_selection_skips_self_contained_nested_handle_boundaries() {
        let output = load_output(&load_fixture(
            "effect_facts",
            "nested_handle_self_contained_vs_outward.scoop",
        ));
        let self_contained = callable(&output, "sample.nested_self_contained");
        let outward = callable(&output, "sample.nested_may_suspend_outward");

        assert!(site_boundaries(self_contained, BoundarySiteKind::Handle).is_empty());
        assert_eq!(site_boundaries(outward, BoundarySiteKind::Handle).len(), 1);
    }

    #[test]
    fn refactor_late_segmentation_splits_statement_boundaries_into_suffix_resume_states() {
        let output = load_output(&load_fixture(
            "effect_facts",
            "dynamic_fallback_widening.scoop",
        ));
        let callable = callable(&output, "sample.callValue");
        let boundary = site_boundaries(callable, BoundarySiteKind::Call)
            .into_iter()
            .next()
            .expect("callValue 应包含 effectful call boundary");

        let owner_state = callable
            .state_graph()
            .state(boundary.owner_state())
            .expect("owner state 应存在");
        let resume_state = callable
            .state_graph()
            .state(boundary.resume_state())
            .expect("resume state 应存在");

        assert_eq!(owner_state.role(), LateLoweredStateRole::Entry);
        assert_eq!(owner_state.source_slices().len(), 1);
        assert_eq!(owner_state.source_slices()[0].start_statement_index(), 0);
        assert_eq!(owner_state.source_slices()[0].end_statement_index(), 1);
        assert!(!owner_state.source_slices()[0].includes_terminator());

        assert_eq!(resume_state.role(), LateLoweredStateRole::Resume);
        assert_eq!(resume_state.source_slices().len(), 1);
        assert_eq!(resume_state.source_slices()[0].start_statement_index(), 1);
        assert_eq!(resume_state.source_slices()[0].end_statement_index(), 1);
        assert!(resume_state.source_slices()[0].includes_terminator());
    }

    #[test]
    fn refactor_late_segmentation_keeps_expression_argument_and_if_context_boundaries_distinct() {
        let output = load_output(&load_fixture(
            "mir_refactor",
            "effect_boundary_inside_expr_context.scoop",
        ));
        let callable = callable(&output, "fixtures.mir_refactor.main");
        let perform_boundaries = site_boundaries(callable, BoundarySiteKind::Perform);

        assert_eq!(perform_boundaries.len(), 4);
        assert_eq!(
            perform_boundaries
                .iter()
                .map(|boundary| boundary.owner_state())
                .collect::<BTreeSet<_>>()
                .len(),
            4,
            "nested expr/arg/if context 里的 boundary 不应折叠到同一个 owner state"
        );
        assert_eq!(
            perform_boundaries
                .iter()
                .map(|boundary| boundary.resume_state())
                .collect::<BTreeSet<_>>()
                .len(),
            4,
            "nested expr/arg/if context 里的 boundary 不应折叠到同一个 resume state"
        );
        assert!(
            callable.state_graph().states().len() > perform_boundaries.len() + 2,
            "整个函数 CFG 在 boundary 递归切分后应保留额外的 segment skeleton"
        );
    }

    #[test]
    fn refactor_owner_resume_state_tracks_loop_condition_boundaries() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_segment_loop_condition.scoop",
            r#"
package sample

effect Flag {
    fun read(): Bool
}

fun cleanup() {}

fun main(): Int {
    while (handle {
        Flag.read()
        false
    } with {
        Flag.read() -> true
    } finally {
        cleanup()
    }) {
        return 1
    }

    return 0
}
"#,
        ));
        let callable = callable(&output, "sample.main");
        let perform_boundaries = site_boundaries(callable, BoundarySiteKind::Perform);

        assert_eq!(perform_boundaries.len(), 1);
        let boundary = perform_boundaries[0];
        assert_ne!(boundary.owner_state(), boundary.resume_state());
        assert!(
            callable.state_graph().states().len() >= 4,
            "loop condition boundary 应显式扩成多个 state skeleton"
        );
    }

    #[test]
    fn refactor_owner_resume_state_keeps_no_outward_callables_plain_without_state_framework() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_segment_no_outward.scoop",
            "package sample\nfun helper() {}\nfun main() { helper() }\n",
        ));
        let callable = callable(&output, "sample.main");

        assert_eq!(callable.call_abi_kind(), CallableAbiKind::Plain);
        assert!(callable.effect_step_abi().is_none());
        let plain = callable
            .plain_abi()
            .expect("NoOutward callable 应保持 plain ABI handoff");
        assert_eq!(plain.body_slices().len(), 1);
        assert!(plain.body_slices()[0].includes_terminator());
    }

    #[test]
    fn refactor_late_control_flow_encodes_loop_break_continue_as_explicit_state_edges() {
        let output = load_output(&SourceFile::new_virtual(
            "<mem>/late_segment_effectful_while_break_continue.scoop",
            r#"
package sample

effect Tick {
    fun read(): Bool
}

fun worker(): Int / Tick {
    while (Tick.read()) {
        if (Tick.read()) {
            break
        } else {
            continue
        }
    }

    return 0
}

fun main(): Int {
    return 0
}
"#,
        ));
        let callable = callable(&output, "sample.worker");
        let states = callable.state_graph().states();

        let branch_state = states
            .iter()
            .find(|state| {
                matches!(
                    state.terminator(),
                    crate::effect_lowered::ir::LateLoweredStateTerminator::Branch { .. }
                )
            })
            .expect("while loop 应在 late-lowered graph 中保留显式 branch state");
        assert!(
            callable
                .state_graph()
                .state(branch_state.state_id())
                .is_some()
        );
        assert!(
            states.iter().any(|state| {
                matches!(
                    state.terminator(),
                    crate::effect_lowered::ir::LateLoweredStateTerminator::Return { .. }
                )
            }),
            "break/loop-exit path 最终应收口为显式 return/complete contract"
        );
    }

    #[test]
    fn refactor_late_control_flow_keeps_handle_body_arm_finally_and_cleanup_edges_explicit() {
        let output = load_output(&load_fixture(
            "mir_refactor",
            "handle_finally_boundary.scoop",
        ));
        let callable = callable(&output, "fixtures.mir_refactor.handled_raise");
        let states = callable.state_graph().states();

        let handle_dispatch = states
            .iter()
            .find_map(|state| match state.terminator() {
                crate::effect_lowered::ir::LateLoweredStateTerminator::HandleDispatch {
                    body_state,
                    arm_states,
                    finally_state,
                    exit_state,
                    ..
                } => Some((*body_state, arm_states.clone(), *finally_state, *exit_state)),
                _ => None,
            })
            .expect("handle 入口应保留显式 HandleDispatch terminator");

        assert!(callable.state_graph().state(handle_dispatch.0).is_some());
        assert!(
            !handle_dispatch.1.is_empty(),
            "handler arm 续点应显式可追踪"
        );
        assert!(
            handle_dispatch
                .2
                .and_then(|state| callable.state_graph().state(state))
                .is_some(),
            "finally/cleanup 续点应显式可追踪"
        );
        assert!(callable.state_graph().state(handle_dispatch.3).is_some());

        assert!(
            states.iter().any(|state| {
                matches!(
                    state.terminator(),
                    crate::effect_lowered::ir::LateLoweredStateTerminator::Suspend {
                        cleanup_state: Some(_),
                        ..
                    }
                )
            }),
            "effectful handle body 内的 outward boundary 应显式记录 cleanup edge"
        );
    }

    #[test]
    fn refactor_dropped_continuation_uses_dedicated_drop_state_instead_of_cleanup() {
        let output = load_output(&load_fixture(
            "effect_lowered",
            "dropped_continuation_abandons_remaining_work.scoop",
        ));
        let callable = callable(&output, "sample.helper");
        let drop_state = callable
            .state_graph()
            .drop_state()
            .expect("outward callable 应发布显式 drop state");
        let drop_node = callable
            .state_graph()
            .state(drop_state)
            .expect("drop state 应可回查");

        assert_eq!(drop_node.role(), LateLoweredStateRole::Drop);
        assert!(matches!(
            drop_node.terminator(),
            crate::effect_lowered::ir::LateLoweredStateTerminator::Abandon
        ));
        assert!(
            callable.state_graph().states().iter().any(|state| {
                matches!(
                    state.terminator(),
                    crate::effect_lowered::ir::LateLoweredStateTerminator::Suspend {
                        cleanup_state: Some(cleanup_state),
                        drop_state: Some(explicit_drop_state),
                        ..
                    } if *cleanup_state != drop_state && *explicit_drop_state == drop_state
                )
            }),
            "dropped continuation 应走独立 drop path，而不是复用 pending cleanup path"
        );
    }

    #[test]
    fn refactor_runtime_error_boundary_stays_inside_explicit_suspend_contract() {
        let output = load_output(&load_fixture(
            "effect_lowered",
            "continuation_resume_runtime_error_boundary.scoop",
        ));
        let callable = callable(&output, "sample.helper");
        let runtime_error_boundary = callable
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    LateLoweredBoundarySource::RuntimeError { .. }
                )
            })
            .expect("resume helper 应发布 ordinary runtime error boundary");
        let resume_boundary = callable
            .boundary_map()
            .entries()
            .iter()
            .find(|boundary| {
                matches!(
                    boundary.source(),
                    LateLoweredBoundarySource::Site {
                        kind: BoundarySiteKind::Resume,
                        ..
                    }
                )
            })
            .expect("resume helper 应发布显式 resume boundary");

        assert_eq!(
            runtime_error_boundary.owner_state(),
            resume_boundary.owner_state()
        );
        assert_eq!(
            runtime_error_boundary.resume_state(),
            resume_boundary.resume_state()
        );
        assert!(
            callable.state_graph().states().iter().any(|state| {
                matches!(
                    state.terminator(),
                    crate::effect_lowered::ir::LateLoweredStateTerminator::Suspend { boundary_ids, .. }
                        if boundary_ids.contains(&runtime_error_boundary.boundary_id())
                            && boundary_ids.contains(&resume_boundary.boundary_id())
                )
            }),
            "resume site 与其 runtime error outward 应共用同一个显式 suspend contract"
        );
    }

    #[test]
    fn refactor_owner_resume_state_builder_consumes_only_p4_facts_and_mir_shape() {
        let session = refactor_session();
        let source = load_fixture("effect_facts", "dynamic_fallback_widening.scoop");
        let effect_lowered_output = load_effect_lowered_stage_output_for_dump(&session, &source)
            .expect("fixture 应可通过 refactor late-lowering stage");
        let pass_view = effect_lowered_output.materialized_pass_view();
        let family = pass_view
            .root_family_for_fqn("sample.callValue")
            .expect("sample.callValue 应有 canonical family");
        let root_fun = family
            .root_body()
            .expect("sample.callValue 应有 canonical root fun");
        let body = root_fun
            .body
            .as_ref()
            .expect("sample.callValue 应有 canonical body");
        let body_facts = effect_lowered_output
            .effect_facts()
            .body(family.key())
            .expect("sample.callValue 应有 P4 body facts");

        let segmentation =
            build_callable_segmentation("sample.callValue", body, body_facts, root_fun.return_ty)
                .expect("segmentation builder 应直接消费 canonical body + body facts");
        assert_eq!(segmentation.boundary_map.entries().len(), 1);
        assert_eq!(segmentation.resume_state_map.entries().len(), 1);
    }
}

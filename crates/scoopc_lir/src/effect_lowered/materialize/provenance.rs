//! Cross-callable continuation provenance, route helpers and impls of materialize types.

#![allow(dead_code)]

use super::*;

#[derive(Debug, Clone)]
pub(crate) struct ResolvedResumeLocalRoute {
    pub(crate) route: Option<LateLoweredContinuationRoute>,
    pub(crate) compatible_route_set: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PublishedMemberStoreRoute {
    None,
    Ambiguous,
    Unique(StoredContinuationValueRoute),
    Resolved {
        path: Vec<PatternBindingStep>,
        route: LateLoweredContinuationRoute,
    },
}

impl PublishedMemberStoreRoute {
    pub(crate) fn from_mir(publication: StoredContinuationRoutePublication) -> Self {
        match publication {
            StoredContinuationRoutePublication::None => Self::None,
            StoredContinuationRoutePublication::Ambiguous => Self::Ambiguous,
            StoredContinuationRoutePublication::Unique(route) => Self::Unique(route),
        }
    }
}

impl PublishedContinuationProvenance {
    pub(crate) fn build(
        root_fqn: &str,
        body: &Body,
        body_facts: &BodyEffectFacts,
        owner_version_key: &LateLoweredBodyVersionKey,
        continuation_object: ContinuationObjectId,
        cross_callable: Option<&CrossCallableContinuationProvenance>,
    ) -> Result<Self, EffectLoweringError> {
        let mut provenance = Self::default();
        let global_value_origins = build_global_value_origins(body);

        for (&site_id, site_facts) in body_facts.sites() {
            let SiteEffectFacts::Handle(handle_facts) = site_facts else {
                continue;
            };
            let handle_arms = lookup_handle_arms(root_fqn, body, site_id)?;
            if handle_arms.len() != handle_facts.arm_facts().len() {
                return Err(invalid_handle_dispatch_contract(
                    root_fqn,
                    site_id,
                    format!(
                        "canonical MIR handle arm 数量({}) 与 HandleSiteEffectFacts.arm_facts 数量({}) 不一致，无法为 continuation provenance 建 seed route",
                        handle_arms.len(),
                        handle_facts.arm_facts().len(),
                    ),
                ));
            }
            for (arm_ordinal, (arm, arm_facts)) in handle_arms
                .iter()
                .zip(handle_facts.arm_facts().iter())
                .enumerate()
            {
                let Some(local) = arm.continuation_local else {
                    continue;
                };
                push_local_origin(
                    &mut provenance.local_origins,
                    local,
                    LocalContinuationOrigin::Seed(LateLoweredContinuationRoute::new(
                        arm_facts.continuation_schema(),
                        LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                            owner_version_key: owner_version_key.clone(),
                            owner_continuation_object: continuation_object,
                            site_id,
                            arm_ordinal: arm_ordinal as u32,
                            handled_case: arm_facts.handled_case(),
                        },
                    )),
                );
            }
        }

        for block in &body.blocks {
            for stmt in &block.stmts {
                match &stmt.kind {
                    StatementKind::Assign {
                        target,
                        value: Rvalue::Use(Operand::Local(source)),
                    } => {
                        push_local_origin(
                            &mut provenance.local_origins,
                            *target,
                            LocalContinuationOrigin::Copy(*source),
                        );
                    }
                    StatementKind::Assign {
                        target,
                        value:
                            Rvalue::EnumVariant {
                                variant_name, args, ..
                            },
                    } => {
                        for (field_index, arg) in args.iter().enumerate() {
                            let Operand::Local(source) = &arg.value else {
                                continue;
                            };
                            push_local_origin(
                                &mut provenance.local_origins,
                                *target,
                                LocalContinuationOrigin::AggregateElement {
                                    source: *source,
                                    path: vec![PatternBindingStep::VariantField {
                                        variant: variant_name.clone(),
                                        field_index,
                                    }],
                                },
                            );
                        }
                    }
                    StatementKind::Assign {
                        target,
                        value: Rvalue::MakeTuple { elements, .. },
                    } => {
                        for (field_index, element) in elements.iter().enumerate() {
                            let Operand::Local(source) = element else {
                                continue;
                            };
                            push_local_origin(
                                &mut provenance.local_origins,
                                *target,
                                LocalContinuationOrigin::AggregateElement {
                                    source: *source,
                                    path: vec![PatternBindingStep::TupleIndex(field_index)],
                                },
                            );
                        }
                    }
                    StatementKind::Assign {
                        target,
                        value:
                            Rvalue::MemberAccess {
                                receiver: Operand::Local(receiver_local),
                                member,
                                ..
                            },
                    } => {
                        push_local_origin(
                            &mut provenance.local_origins,
                            *target,
                            LocalContinuationOrigin::MemberRead(
                                ContinuationMemberKey::from_metadata(*receiver_local, member),
                            ),
                        );
                    }
                    StatementKind::StoreMember {
                        receiver: Operand::Local(receiver_local),
                        member,
                        continuation_route,
                        ..
                    } => {
                        provenance
                            .member_store_routes
                            .entry(ContinuationMemberKey::from_metadata(
                                *receiver_local,
                                member,
                            ))
                            .or_default()
                            .push(PublishedMemberStoreRoute::from_mir(
                                continuation_route.clone(),
                            ));
                    }
                    StatementKind::Nop
                    | StatementKind::Todo(_)
                    | StatementKind::Assign { .. }
                    | StatementKind::StoreMember { .. }
                    | StatementKind::StoreTopLevelVar { .. } => {}
                }
            }
        }

        if let Some(cross_callable) = cross_callable {
            provenance.add_cross_callable_member_routes(
                body,
                cross_callable,
                &global_value_origins,
            );
        }

        for block in &body.blocks {
            for stmt in &block.stmts {
                let StatementKind::Assign {
                    target,
                    value:
                        Rvalue::PatternExtract {
                            subject: Operand::Local(subject),
                            path,
                        },
                } = &stmt.kind
                else {
                    continue;
                };
                push_local_origin(
                    &mut provenance.local_origins,
                    *target,
                    LocalContinuationOrigin::PatternExtract {
                        subject: *subject,
                        path: path.clone(),
                    },
                );
                let Some((key, mut prefix_path)) = member_derived_origin_for_local(
                    *subject,
                    &provenance.local_origins,
                    &mut HashSet::new(),
                ) else {
                    continue;
                };
                prefix_path.extend(path.iter().cloned());
                push_local_origin(
                    &mut provenance.local_origins,
                    *target,
                    LocalContinuationOrigin::PatternMemberRead {
                        key,
                        path: prefix_path,
                    },
                );
            }
        }

        Ok(provenance)
    }

    pub(crate) fn add_cross_callable_member_routes(
        &mut self,
        body: &Body,
        cross_callable: &CrossCallableContinuationProvenance,
        global_value_origins: &HashMap<LocalId, String>,
    ) {
        for block in &body.blocks {
            for stmt in &block.stmts {
                if let StatementKind::Assign {
                    target: _,
                    value:
                        Rvalue::MemberAccess {
                            receiver: Operand::Local(receiver_local),
                            member,
                            ..
                        },
                } = &stmt.kind
                    && let Some(receiver_fqn) = global_value_origins.get(receiver_local)
                {
                    let key = ContinuationMemberKey::from_metadata(*receiver_local, member);
                    for route in cross_callable.routes_for_global_member(receiver_fqn, &key.member)
                    {
                        self.member_store_routes
                            .entry(key.clone())
                            .or_default()
                            .push(PublishedMemberStoreRoute::Resolved {
                                path: route.path.clone(),
                                route: route.route.clone(),
                            });
                    }
                }

                let StatementKind::Assign {
                    value:
                        Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn, .. },
                            args,
                            ..
                        },
                    ..
                } = &stmt.kind
                else {
                    continue;
                };
                for route in cross_callable.routes_for_callee(callee_fqn) {
                    let Some(arg) = args.get(route.param_index) else {
                        continue;
                    };
                    let Operand::Local(receiver_local) = &arg.value else {
                        continue;
                    };
                    let key = ContinuationMemberKey {
                        receiver_local: *receiver_local,
                        member: route.member.clone(),
                    };
                    self.member_store_routes.entry(key).or_default().push(
                        PublishedMemberStoreRoute::Resolved {
                            path: route.path.clone(),
                            route: route.route.clone(),
                        },
                    );
                }
            }
        }
    }

    pub(crate) fn resolve_resume_local_route(
        &self,
        root_fqn: &str,
        site_id: SiteId,
        local: LocalId,
    ) -> Result<ResolvedResumeLocalRoute, EffectLoweringError> {
        let routes = self.resolve_local_routes(
            root_fqn,
            site_id,
            local,
            &mut HashSet::new(),
            &mut HashSet::new(),
        )?;
        match routes.as_slice() {
            [] => Ok(ResolvedResumeLocalRoute {
                route: None,
                compatible_route_set: false,
            }),
            [route] => Ok(ResolvedResumeLocalRoute {
                route: Some(route.clone()),
                compatible_route_set: false,
            }),
            routes if routes_share_dynamic_resume_shape(routes) => Ok(ResolvedResumeLocalRoute {
                route: routes.first().cloned(),
                compatible_route_set: true,
            }),
            _ => Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                format!(
                    "continuation local{} 通过 published member write/read route 同时解析到多条互不兼容的 underlying continuation route",
                    local.as_u32(),
                ),
            )),
        }
    }

    pub(crate) fn resolve_local_routes(
        &self,
        root_fqn: &str,
        site_id: SiteId,
        local: LocalId,
        visiting_locals: &mut HashSet<LocalId>,
        visiting_members: &mut HashSet<(ContinuationMemberKey, Vec<PatternBindingStep>)>,
    ) -> Result<Vec<LateLoweredContinuationRoute>, EffectLoweringError> {
        if !visiting_locals.insert(local) {
            return Ok(Vec::new());
        }
        let mut routes = Vec::new();
        if let Some(origins) = self.local_origins.get(&local) {
            for origin in origins {
                match origin {
                    LocalContinuationOrigin::Seed(route) => {
                        push_unique_route(&mut routes, route.clone());
                    }
                    LocalContinuationOrigin::Copy(source) => {
                        for route in self.resolve_local_routes(
                            root_fqn,
                            site_id,
                            *source,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::AggregateElement { .. } => {}
                    LocalContinuationOrigin::MemberRead(key) => {
                        for route in self.resolve_member_path_routes(
                            root_fqn,
                            site_id,
                            key,
                            &[],
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::PatternExtract { subject, path } => {
                        for route in self.resolve_local_pattern_routes(
                            root_fqn,
                            site_id,
                            *subject,
                            path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::PatternMemberRead { key, path } => {
                        for route in self.resolve_member_path_routes(
                            root_fqn,
                            site_id,
                            key,
                            path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                }
            }
        }
        visiting_locals.remove(&local);
        Ok(routes)
    }

    pub(crate) fn resolve_local_pattern_routes(
        &self,
        root_fqn: &str,
        site_id: SiteId,
        local: LocalId,
        path: &[PatternBindingStep],
        visiting_locals: &mut HashSet<LocalId>,
        visiting_members: &mut HashSet<(ContinuationMemberKey, Vec<PatternBindingStep>)>,
    ) -> Result<Vec<LateLoweredContinuationRoute>, EffectLoweringError> {
        if !visiting_locals.insert(local) {
            return Ok(Vec::new());
        }
        let mut routes = Vec::new();
        if let Some(origins) = self.local_origins.get(&local) {
            for origin in origins {
                match origin {
                    LocalContinuationOrigin::Seed(route) if path.is_empty() => {
                        push_unique_route(&mut routes, route.clone());
                    }
                    LocalContinuationOrigin::Seed(_) => {}
                    LocalContinuationOrigin::Copy(source) => {
                        for route in self.resolve_local_pattern_routes(
                            root_fqn,
                            site_id,
                            *source,
                            path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::AggregateElement {
                        source,
                        path: element_path,
                    } => {
                        let Some(remaining_path) = path.strip_prefix(element_path.as_slice())
                        else {
                            continue;
                        };
                        let source_routes = if remaining_path.is_empty() {
                            self.resolve_local_routes(
                                root_fqn,
                                site_id,
                                *source,
                                visiting_locals,
                                visiting_members,
                            )?
                        } else {
                            self.resolve_local_pattern_routes(
                                root_fqn,
                                site_id,
                                *source,
                                remaining_path,
                                visiting_locals,
                                visiting_members,
                            )?
                        };
                        for route in source_routes {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::MemberRead(key) => {
                        for route in self.resolve_member_path_routes(
                            root_fqn,
                            site_id,
                            key,
                            path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::PatternExtract {
                        subject,
                        path: prefix_path,
                    } => {
                        let mut combined_path = prefix_path.clone();
                        combined_path.extend_from_slice(path);
                        for route in self.resolve_local_pattern_routes(
                            root_fqn,
                            site_id,
                            *subject,
                            &combined_path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                    LocalContinuationOrigin::PatternMemberRead {
                        key,
                        path: prefix_path,
                    } => {
                        let mut combined_path = prefix_path.clone();
                        combined_path.extend_from_slice(path);
                        for route in self.resolve_member_path_routes(
                            root_fqn,
                            site_id,
                            key,
                            &combined_path,
                            visiting_locals,
                            visiting_members,
                        )? {
                            push_unique_route(&mut routes, route);
                        }
                    }
                }
            }
        }
        visiting_locals.remove(&local);
        Ok(routes)
    }

    pub(crate) fn resolve_member_path_routes(
        &self,
        root_fqn: &str,
        site_id: SiteId,
        key: &ContinuationMemberKey,
        path: &[PatternBindingStep],
        visiting_locals: &mut HashSet<LocalId>,
        visiting_members: &mut HashSet<(ContinuationMemberKey, Vec<PatternBindingStep>)>,
    ) -> Result<Vec<LateLoweredContinuationRoute>, EffectLoweringError> {
        let cycle_key = (key.clone(), path.to_vec());
        if !visiting_members.insert(cycle_key.clone()) {
            return Ok(Vec::new());
        }

        let publications = self.member_store_routes.get(key).ok_or_else(|| {
            invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                format!(
                    "member {} 没有任何 published member write contract，无法把 readback route 接回 continuation provenance",
                    render_continuation_member_key(key),
                ),
            )
        })?;

        let mut routes = Vec::new();
        let mut saw_ambiguous_publication = false;
        let mut saw_matching_publication = false;
        for publication in publications {
            match publication {
                PublishedMemberStoreRoute::None => {}
                PublishedMemberStoreRoute::Ambiguous => {
                    saw_ambiguous_publication = true;
                }
                PublishedMemberStoreRoute::Unique(route) if route.path == path => {
                    saw_matching_publication = true;
                    let source_routes = self.resolve_local_routes(
                        root_fqn,
                        site_id,
                        route.source_local,
                        visiting_locals,
                        visiting_members,
                    )?;
                    if source_routes.is_empty() {
                        visiting_members.remove(&cycle_key);
                        return Err(invalid_boundary_operand_contract(
                            root_fqn,
                            site_id,
                            "Resume",
                            format!(
                                "member {} 的 published write path {} 指向 local{}，但该 source local 没有已发布的 continuation route",
                                render_continuation_member_key(key),
                                render_pattern_path(path),
                                route.source_local.as_u32(),
                            ),
                        ));
                    }
                    for source_route in source_routes {
                        push_unique_route(&mut routes, source_route);
                    }
                }
                PublishedMemberStoreRoute::Resolved {
                    path: route_path,
                    route,
                } if route_path == path => {
                    saw_matching_publication = true;
                    push_unique_route(&mut routes, route.clone());
                }
                PublishedMemberStoreRoute::Unique(_)
                | PublishedMemberStoreRoute::Resolved { .. } => {}
            }
        }

        visiting_members.remove(&cycle_key);

        if saw_ambiguous_publication {
            return Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                format!(
                    "member {} 的 published member write contract 标记为 Ambiguous，无法唯一确定 readback path {} 的 continuation provenance",
                    render_continuation_member_key(key),
                    render_pattern_path(path),
                ),
            ));
        }
        if !saw_matching_publication || routes.is_empty() {
            return Err(invalid_boundary_operand_contract(
                root_fqn,
                site_id,
                "Resume",
                format!(
                    "member {} 没有与 readback path {} 对齐的 published continuation write/read provenance",
                    render_continuation_member_key(key),
                    render_pattern_path(path),
                ),
            ));
        }

        Ok(routes)
    }
}

pub(crate) fn build_cross_callable_continuation_provenance(
    pass_view: &MaterializedMirPassView<'_>,
    effect_facts: &MaterializedEffectFacts,
    owner_plans: &HashMap<String, ContinuationRouteOwnerPlan>,
) -> Result<CrossCallableContinuationProvenance, EffectLoweringError> {
    let mut member_routes_by_callee: HashMap<String, Vec<CrossCallableContinuationMemberRoute>> =
        HashMap::new();
    let mut global_member_routes: HashMap<
        GlobalContinuationMemberKey,
        Vec<CrossCallableGlobalContinuationMemberRoute>,
    > = HashMap::new();

    for family in pass_view.instances() {
        let root_fqn = family.root_fqn();
        let Some(fun) = family.root_body() else {
            continue;
        };
        let Some(body) = fun.body.as_ref() else {
            continue;
        };
        let Some(body_facts) = effect_facts.body(family.key()) else {
            continue;
        };
        let Some(owner_plan) = owner_plans.get(root_fqn) else {
            continue;
        };
        let provenance = PublishedContinuationProvenance::build(
            root_fqn,
            body,
            body_facts,
            &owner_plan.owner_version_key,
            owner_plan.continuation_object,
            None,
        )?;
        let global_value_origins = build_global_value_origins(body);

        for (key, publications) in &provenance.member_store_routes {
            let global_receiver = global_value_origins.get(&key.receiver_local).cloned();
            let param_index = fun
                .params
                .iter()
                .position(|param| param.local == key.receiver_local);
            for publication in publications {
                let PublishedMemberStoreRoute::Unique(route) = publication else {
                    continue;
                };
                let source_routes = provenance.resolve_local_routes(
                    root_fqn,
                    SiteId::from_raw(u32::MAX),
                    route.source_local,
                    &mut HashSet::new(),
                    &mut HashSet::new(),
                )?;
                for source_route in source_routes {
                    if let Some(param_index) = param_index {
                        member_routes_by_callee
                            .entry(root_fqn.to_string())
                            .or_default()
                            .push(CrossCallableContinuationMemberRoute {
                                param_index,
                                member: key.member.clone(),
                                path: route.path.clone(),
                                route: source_route.clone(),
                            });
                    }
                    if let Some(receiver_fqn) = &global_receiver {
                        global_member_routes
                            .entry(GlobalContinuationMemberKey {
                                receiver_fqn: receiver_fqn.clone(),
                                member: key.member.clone(),
                            })
                            .or_default()
                            .push(CrossCallableGlobalContinuationMemberRoute {
                                path: route.path.clone(),
                                route: source_route,
                            });
                    }
                }
            }
        }
    }

    Ok(CrossCallableContinuationProvenance {
        member_routes_by_callee,
        global_member_routes,
    })
}

pub(crate) fn build_global_value_origins(body: &Body) -> HashMap<LocalId, String> {
    let mut origins = HashMap::new();
    for block in &body.blocks {
        for stmt in &block.stmts {
            let StatementKind::Assign { target, value } = &stmt.kind else {
                continue;
            };
            match value {
                Rvalue::TopLevelRef(top) => {
                    origins.insert(*target, top.fqn.clone());
                }
                Rvalue::Use(Operand::Local(source))
                | Rvalue::Transport {
                    value: Operand::Local(source),
                    ..
                } => {
                    if let Some(origin) = origins.get(source).cloned() {
                        origins.insert(*target, origin);
                    }
                }
                Rvalue::MemberAccess {
                    receiver: Operand::Local(receiver),
                    member,
                    ..
                } => {
                    let Some(receiver_origin) = origins.get(receiver) else {
                        continue;
                    };
                    let Some(member_fqn) = member_value_fqn(member) else {
                        continue;
                    };
                    if member_fqn.starts_with(receiver_origin)
                        && member_fqn.as_bytes().get(receiver_origin.len()) == Some(&b'.')
                    {
                        origins.insert(*target, member_fqn.to_string());
                    }
                }
                _ => {}
            }
        }
    }
    origins
}

pub(crate) fn member_value_fqn(member: &MemberAccessMetadata) -> Option<&str> {
    match member.resolved.as_ref()? {
        MemberTarget::Value { fqn } | MemberTarget::ExtensionValue { fqn } => Some(fqn.as_str()),
        MemberTarget::Fun { .. } | MemberTarget::ExtensionFun { .. } => None,
    }
}

pub(crate) fn push_local_origin(
    origins: &mut HashMap<LocalId, Vec<LocalContinuationOrigin>>,
    local: LocalId,
    origin: LocalContinuationOrigin,
) {
    let entry = origins.entry(local).or_default();
    if !entry.contains(&origin) {
        entry.push(origin);
    }
}

pub(crate) fn push_unique_route(
    routes: &mut Vec<LateLoweredContinuationRoute>,
    route: LateLoweredContinuationRoute,
) {
    if !routes.contains(&route) {
        routes.push(route);
    }
}

pub(crate) fn routes_share_dynamic_resume_shape(routes: &[LateLoweredContinuationRoute]) -> bool {
    let Some(first) = routes.first() else {
        return false;
    };
    routes
        .iter()
        .skip(1)
        .all(|route| same_dynamic_resume_route_shape(first, route))
}

pub(crate) fn same_dynamic_resume_route_shape(
    left: &LateLoweredContinuationRoute,
    right: &LateLoweredContinuationRoute,
) -> bool {
    if left.continuation_schema() != right.continuation_schema() {
        return false;
    }
    match (left.publication(), right.publication()) {
        (
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. },
            LateLoweredSurfaceResumeDispatchPublication::ResumeBoundary { .. },
        ) => true,
        (
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_version_key: left_owner,
                owner_continuation_object: left_object,
                ..
            },
            LateLoweredSurfaceResumeDispatchPublication::HandleContinuationBinder {
                owner_version_key: right_owner,
                owner_continuation_object: right_object,
                ..
            },
        ) => left_owner == right_owner && left_object == right_object,
        _ => false,
    }
}

pub(crate) fn member_derived_origin_for_local(
    local: LocalId,
    local_origins: &HashMap<LocalId, Vec<LocalContinuationOrigin>>,
    visiting: &mut HashSet<LocalId>,
) -> Option<(ContinuationMemberKey, Vec<PatternBindingStep>)> {
    if !visiting.insert(local) {
        return None;
    }
    let mut resolved = None;
    for origin in local_origins.get(&local)? {
        let next = match origin {
            LocalContinuationOrigin::MemberRead(key) => Some((key.clone(), Vec::new())),
            LocalContinuationOrigin::PatternMemberRead { key, path } => {
                Some((key.clone(), path.clone()))
            }
            LocalContinuationOrigin::Copy(source) => {
                member_derived_origin_for_local(*source, local_origins, visiting)
            }
            LocalContinuationOrigin::Seed(_)
            | LocalContinuationOrigin::AggregateElement { .. }
            | LocalContinuationOrigin::PatternExtract { .. } => None,
        };
        let Some(next) = next else {
            continue;
        };
        match &resolved {
            Some(existing) if existing != &next => {
                visiting.remove(&local);
                return None;
            }
            Some(_) => {}
            None => resolved = Some(next),
        }
    }
    visiting.remove(&local);
    resolved
}

pub(crate) fn render_continuation_member_key(key: &ContinuationMemberKey) -> String {
    let member = match &key.member {
        ContinuationMemberIdentityKey::Value(fqn)
        | ContinuationMemberIdentityKey::Fun(fqn)
        | ContinuationMemberIdentityKey::ExtensionValue(fqn)
        | ContinuationMemberIdentityKey::ExtensionFun(fqn) => fqn.clone(),
        ContinuationMemberIdentityKey::Unresolved { name, receiver_ty } => {
            format!("{}.{}", receiver_ty.as_u32(), name)
        }
    };
    format!("local{}.{}", key.receiver_local.as_u32(), member)
}

pub(crate) fn render_pattern_path(path: &[PatternBindingStep]) -> String {
    if path.is_empty() {
        return "<identity>".to_string();
    }
    path.iter()
        .map(|step| match step {
            PatternBindingStep::TupleIndex(index) => format!("tuple[{index}]"),
            PatternBindingStep::VariantField {
                variant,
                field_index,
            } => format!("{variant}[{field_index}]"),
        })
        .collect::<Vec<_>>()
        .join(" -> ")
}

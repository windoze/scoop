use super::*;

pub(super) fn value_layout(
    value: &crate::ExactValueLayoutV1,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    exact_record(
        value.identity(),
        |candidate| candidate.value_handle().as_deref() == Some(value),
        depth,
        views,
        index,
        pending,
        meter,
    )
}

pub(super) fn instance_layout(
    value: &crate::ExactInstanceLayoutV1,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    exact_record(
        value.identity(),
        |candidate| candidate.instance_handle().as_deref() == Some(value),
        depth,
        views,
        index,
        pending,
        meter,
    )
}

pub(super) fn constituent(
    value: &crate::ValueLayoutConstituentV1,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let owner = exact_target(value.layout(), None, depth, views, index, pending, meter)?;
    let semantic = LayoutAbiSemanticTargetV1::Layout(value.layout());
    let Some(candidate) = views[owner].layouts().get(value.layout()) else {
        return Err(LayoutAbiSemanticClosureError::MissingTarget(semantic));
    };
    if candidate
        .value_handle()
        .is_some_and(|candidate| candidate.value() == value)
    {
        Ok(())
    } else {
        Err(LayoutAbiSemanticClosureError::EmbeddedRecord(semantic))
    }
}

pub(super) fn descriptor_ref(
    reference: crate::StrongTypeDescriptorRefV2,
    current: ConeIdentity,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let (provider, exact) = match reference {
        crate::StrongTypeDescriptorRefV2::Local(exact) => (current, exact),
        crate::StrongTypeDescriptorRefV2::CoreExternal(exact) => (ConeIdentity::CORE, exact),
        crate::StrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
            (provider, exact)
        }
    };
    target(
        LayoutAbiSemanticTargetV1::Descriptor(exact),
        Some(provider),
        depth,
        views,
        index,
        pending,
        meter,
    )
    .map(|_| ())
}

pub(super) fn embedded_callable(
    record: &crate::ExactCallableAbiExportV1,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let semantic = LayoutAbiSemanticTargetV1::Callable(record.target());
    let owner = target(
        semantic,
        Some(record.physical_definition().provider()),
        depth,
        views,
        index,
        pending,
        meter,
    )?;
    if views[owner].callables().get(record.target()) == Some(record) {
        Ok(())
    } else {
        Err(LayoutAbiSemanticClosureError::EmbeddedRecord(semantic))
    }
}

pub(super) fn exact_target(
    layout: PersistentLayoutId,
    provider: Option<ConeIdentity>,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<usize, LayoutAbiSemanticClosureError> {
    target(
        LayoutAbiSemanticTargetV1::Layout(layout),
        provider,
        depth,
        views,
        index,
        pending,
        meter,
    )
}

fn exact_record(
    identity: &crate::ExactLayoutIdentityV1,
    matches: impl FnOnce(&crate::ExactLayoutExportV1) -> bool,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let semantic = LayoutAbiSemanticTargetV1::Layout(identity.layout());
    let owner = target(
        semantic,
        Some(identity.physical_definition().provider()),
        depth,
        views,
        index,
        pending,
        meter,
    )?;
    let Some(candidate) = views[owner].layouts().get(identity.layout()) else {
        return Err(LayoutAbiSemanticClosureError::MissingTarget(semantic));
    };
    if matches(candidate) {
        Ok(())
    } else {
        Err(LayoutAbiSemanticClosureError::EmbeddedRecord(semantic))
    }
}

pub(super) fn target(
    target: LayoutAbiSemanticTargetV1,
    expected_provider: Option<ConeIdentity>,
    depth: u64,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
    meter: &mut BudgetMeter,
) -> Result<usize, LayoutAbiSemanticClosureError> {
    let owner = index.owner(target, meter)?;
    if let Some(expected) = expected_provider {
        require_provider(views, owner, expected, target)?;
    }
    push(
        pending,
        Pending {
            owner,
            target,
            depth,
        },
        meter,
    )?;
    Ok(owner)
}

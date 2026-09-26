use super::*;

pub(super) fn value_layout(
    value: &crate::ExactValueLayoutV1,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    exact_record(
        value.identity(),
        |candidate| candidate.value_handle().as_deref() == Some(value),
        views,
        index,
        pending,
    )
}

pub(super) fn instance_layout(
    value: &crate::ExactInstanceLayoutV1,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    exact_record(
        value.identity(),
        |candidate| candidate.instance_handle().as_deref() == Some(value),
        views,
        index,
        pending,
    )
}

pub(super) fn descriptor_ref(
    reference: crate::StrongTypeDescriptorRefV2,
    current: ConeIdentity,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let (provider, exact) = match reference {
        crate::StrongTypeDescriptorRefV2::Local(exact) => (current, exact),
        crate::StrongTypeDescriptorRefV2::DependencyExternal { provider, exact } => {
            (provider, exact)
        }
    };
    target(
        LayoutAbiSemanticTargetV1::Descriptor(exact),
        Some(provider),
        views,
        index,
        pending,
    )
    .map(|_| ())
}

fn exact_record(
    identity: &crate::ExactLayoutIdentityV1,
    matches: impl FnOnce(&crate::ExactLayoutExportV1) -> bool,
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<(), LayoutAbiSemanticClosureError> {
    let semantic = LayoutAbiSemanticTargetV1::Layout(identity.layout());
    let owner = target(
        semantic,
        Some(identity.physical_definition().provider()),
        views,
        index,
        pending,
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
    views: &[&LayoutAbiExportConstituentsV1],
    index: &LayoutAbiTargetIndex,
    pending: &mut Vec<Pending>,
) -> Result<usize, LayoutAbiSemanticClosureError> {
    let owner = index.owner(target)?;
    if let Some(expected) = expected_provider {
        require_provider(views, owner, expected, target)?;
    }
    push(pending, Pending { owner, target })?;
    Ok(owner)
}

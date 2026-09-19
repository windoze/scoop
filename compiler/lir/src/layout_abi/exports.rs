use super::*;

/// The five canonical local inventories owned by one complete layout/ABI
/// section. Their constructors retain the underlying physical replay proofs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutAbiExportConstituentsV1 {
    layouts: crate::CanonicalExactLayoutExportsV1,
    descriptors: crate::CanonicalExactDescriptorExportsV1,
    dispatch: crate::CanonicalExactDispatchExportsV1,
    callables: crate::CanonicalExactCallableAbiExportsV1,
    shape_support: crate::CanonicalParamFreeShapeSupportExportsV1,
}

impl LayoutAbiExportConstituentsV1 {
    pub fn try_new(
        layouts: crate::CanonicalExactLayoutExportsV1,
        descriptors: crate::CanonicalExactDescriptorExportsV1,
        dispatch: crate::CanonicalExactDispatchExportsV1,
        callables: crate::CanonicalExactCallableAbiExportsV1,
        shape_support: crate::CanonicalParamFreeShapeSupportExportsV1,
    ) -> Result<Self, LayoutAbiExportConstituentsError> {
        let provider = layouts.provider();
        if [
            descriptors.provider(),
            dispatch.provider(),
            callables.provider(),
            shape_support.provider(),
        ]
        .into_iter()
        .any(|actual| actual != provider)
        {
            return Err(LayoutAbiExportConstituentsError::Provider);
        }
        let target = layouts.target();
        if [
            descriptors.target(),
            dispatch.target(),
            callables.target(),
            shape_support.target(),
        ]
        .into_iter()
        .any(|actual| actual != target)
        {
            return Err(LayoutAbiExportConstituentsError::Target);
        }
        Ok(Self {
            layouts,
            descriptors,
            dispatch,
            callables,
            shape_support,
        })
    }

    pub fn provider(&self) -> ConeIdentity {
        self.layouts.provider()
    }

    pub fn target_profile(&self) -> crate::LirTargetProfile {
        self.layouts.target()
    }

    pub const fn layouts(&self) -> &crate::CanonicalExactLayoutExportsV1 {
        &self.layouts
    }

    pub const fn descriptors(&self) -> &crate::CanonicalExactDescriptorExportsV1 {
        &self.descriptors
    }

    pub const fn dispatch(&self) -> &crate::CanonicalExactDispatchExportsV1 {
        &self.dispatch
    }

    pub const fn callables(&self) -> &crate::CanonicalExactCallableAbiExportsV1 {
        &self.callables
    }

    pub const fn shape_support(&self) -> &crate::CanonicalParamFreeShapeSupportExportsV1 {
        &self.shape_support
    }

    pub fn record(
        &self,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'_>> {
        match target {
            LayoutAbiSemanticTargetV1::Layout(layout) => self
                .layouts
                .get(layout)
                .map(LayoutAbiSemanticRecordV1::Layout),
            LayoutAbiSemanticTargetV1::Descriptor(exact) => self
                .descriptors
                .get(exact)
                .map(LayoutAbiSemanticRecordV1::Descriptor),
            LayoutAbiSemanticTargetV1::Dispatch(table) => self
                .dispatch
                .get(table)
                .map(LayoutAbiSemanticRecordV1::Dispatch),
            LayoutAbiSemanticTargetV1::Callable(target) => self
                .callables
                .get(target)
                .map(LayoutAbiSemanticRecordV1::Callable),
            LayoutAbiSemanticTargetV1::ShapeSupport(source) => self
                .shape_support
                .get(source)
                .map(LayoutAbiSemanticRecordV1::ShapeSupport),
        }
    }

    pub(crate) fn visit_targets<E>(
        &self,
        mut visit: impl FnMut(LayoutAbiSemanticTargetV1) -> Result<(), E>,
    ) -> Result<(), E> {
        for record in self.layouts.records() {
            visit(LayoutAbiSemanticTargetV1::Layout(
                record.identity().layout(),
            ))?;
        }
        for record in self.descriptors.records() {
            visit(LayoutAbiSemanticTargetV1::Descriptor(record.exact()))?;
        }
        for record in self.dispatch.records() {
            visit(LayoutAbiSemanticTargetV1::Dispatch(record.table()))?;
        }
        for record in self.callables.records() {
            visit(LayoutAbiSemanticTargetV1::Callable(record.target()))?;
        }
        for record in self.shape_support.records() {
            visit(LayoutAbiSemanticTargetV1::ShapeSupport(
                record.source_nominal(),
            ))?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum LayoutAbiSemanticRecordV1<'a> {
    Layout(&'a crate::ExactLayoutExportV1),
    Descriptor(&'a crate::ExactDescriptorExportV1),
    Dispatch(&'a crate::ExactDispatchExportV1),
    Callable(&'a crate::ExactCallableAbiExportV1),
    ShapeSupport(&'a crate::ParamFreeShapeSupportExportV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutAbiExportConstituentsError {
    Provider,
    Target,
}

impl std::fmt::Display for LayoutAbiExportConstituentsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid layout/ABI export constituents: {self:?}"
        )
    }
}

impl std::error::Error for LayoutAbiExportConstituentsError {}

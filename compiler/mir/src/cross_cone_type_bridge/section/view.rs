use super::*;

/// Borrowed constituents for dependency graph replay; this is not a selection
/// handle or evidence that source-use and artifact checks have completed.
#[derive(Clone, Copy)]
pub struct MirTypeBridgeDependencyViewV1<'a> {
    pub(super) provider: ConeIdentity,
    pub(super) exports: &'a MirTypeBridgeExportConstituentsV1,
    pub(super) units: &'a [MirTypeBridgeInitializationUnitV1],
    pub(super) legacy: &'a [StrongCallableDefinitionOwner],
}
pub(super) type LocalView<'a> = MirTypeBridgeDependencyViewV1<'a>;
impl<'a> LocalView<'a> {
    pub const fn provider(self) -> ConeIdentity {
        self.provider
    }

    pub(super) fn record(
        self,
        target: MirTypeBridgeTargetV1,
    ) -> Option<MirTypeBridgeSemanticRecordV1<'a>> {
        match target {
            MirTypeBridgeTargetV1::Type(exact) => self
                .exports
                .types()
                .get(exact)
                .map(MirTypeBridgeSemanticRecordV1::Type),
            MirTypeBridgeTargetV1::Callable(target) => self
                .exports
                .callables()
                .get(target)
                .map(MirTypeBridgeSemanticRecordV1::Callable),
            MirTypeBridgeTargetV1::Dispatch(owner) => self
                .exports
                .dispatch()
                .get(owner)
                .map(MirTypeBridgeSemanticRecordV1::Dispatch),
            MirTypeBridgeTargetV1::Object(value) => self
                .exports
                .objects()
                .get(value)
                .map(MirTypeBridgeSemanticRecordV1::Object),
            MirTypeBridgeTargetV1::ShapeSupport(source) => self
                .exports
                .shapes()
                .get(source)
                .map(MirTypeBridgeSemanticRecordV1::ShapeSupport),
            MirTypeBridgeTargetV1::InitializationUnit(unit) => self
                .units
                .binary_search_by_key(&unit, MirTypeBridgeInitializationUnitV1::unit)
                .ok()
                .map(|index| MirTypeBridgeSemanticRecordV1::InitializationUnit(&self.units[index])),
        }
    }
    pub(super) fn targets<E>(
        self,
        mut visit: impl FnMut(MirTypeBridgeTargetV1) -> Result<(), E>,
    ) -> Result<(), E> {
        for record in self.exports.types().records() {
            visit(MirTypeBridgeTargetV1::Type(record.exact()))?;
        }
        for record in self.exports.callables().entries() {
            visit(MirTypeBridgeTargetV1::Callable(record.implementation()))?;
        }
        for record in self.exports.dispatch().records() {
            visit(MirTypeBridgeTargetV1::Dispatch(record.owner()))?;
        }
        for record in self.exports.objects().records() {
            visit(MirTypeBridgeTargetV1::Object(record.value()))?;
        }
        for record in self.exports.shapes().records() {
            visit(MirTypeBridgeTargetV1::ShapeSupport(record.source()))?;
        }
        for record in self.units {
            visit(MirTypeBridgeTargetV1::InitializationUnit(record.unit()))?;
        }
        Ok(())
    }
}

impl CrossConeMirTypeBridgeSectionV1<'_> {
    pub(super) fn view(&self) -> LocalView<'_> {
        LocalView {
            provider: self.provider(),
            exports: &self.exports,
            units: &self.units,
            legacy: &self.legacy_callables,
        }
    }
}

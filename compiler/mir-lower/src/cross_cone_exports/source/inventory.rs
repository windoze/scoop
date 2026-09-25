use super::*;

pub(super) struct Inventory {
    pub types: Vec<PersistentExactTypeId>,
    pub callables: Vec<StrongCallableDefinitionOwner>,
    pub dispatch: Vec<PersistentExactTypeId>,
    pub objects: Vec<PersistentObjectValueId>,
    pub roots: Vec<PersistentTypeId>,
}
impl Inventory {
    pub fn from_source(
        input: MirTypeBridgeExportInputV1<'_>,
        source: &mir::MirTypeBridgeExportConstituentsV1,
    ) -> Result<Self, MirTypeBridgeSourceProjectionError> {
        let mut roots = collect(
            input
                .hir
                .output()
                .local
                .materialization()
                .roots()
                .iter()
                .map(|r| r.source()),
        )?;

        roots.sort_unstable();
        source
            .shapes()
            .validate_required_sources(&roots)
            .map_err(MirTypeBridgeSourceProjectionError::Shapes)?;
        Ok(Self {
            types: collect(source.types().records().iter().map(|r| r.exact()))?,
            callables: collect(
                source
                    .callables()
                    .entries()
                    .iter()
                    .map(|r| r.implementation()),
            )?,
            dispatch: collect(source.dispatch().records().iter().map(|r| r.owner()))?,
            objects: collect(source.objects().records().iter().map(|r| r.value()))?,
            roots,
        })
    }
}

pub(super) fn collect<T>(
    source: impl ExactSizeIterator<Item = T>,
) -> Result<Vec<T>, MirTypeBridgeSourceProjectionError> {
    let count = source.len();
    let path = WirePath::root();

    let mut values = Vec::new();
    scoop_wire::allocation::try_reserve(&mut values, count, &path)?;
    values.extend(source);
    Ok(values)
}

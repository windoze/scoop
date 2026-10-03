//! Structural values need storage, but not a separately exported definition.

use std::collections::BTreeMap;

use scoop_identity::{
    ExactTypeKey, LayoutKey, PersistentExactTypeId, PersistentLayoutId, RepresentationRole,
    ScoopAbiValueShape, ValidatedIdentityGraph,
};

use super::{LinkDataError, link_error};
use crate::*;

pub(crate) type LinkLayouts = BTreeMap<PersistentLayoutId, ExactLayoutExportV1>;

#[derive(Clone)]
pub(crate) struct LinkValueStorage {
    pub value: ValueLayoutConstituentV1,
    pub shape: ScoopAbiValueShape,
    pub pointer: Option<NichePointerKind>,
}

#[derive(Default)]
pub(crate) struct ValueStorageReader {
    values: BTreeMap<PersistentExactTypeId, LinkValueStorage>,
}

impl ValueStorageReader {
    pub fn read(
        &mut self,
        exact: PersistentExactTypeId,
        target: LirTargetProfile,
        identities: &ValidatedIdentityGraph,
        layouts: &LinkLayouts,
    ) -> Result<LinkValueStorage, LinkDataError> {
        if let Some(value) = self.values.get(&exact) {
            return Ok(value.clone());
        }
        let key = identities
            .canonical_record::<_, ExactTypeKey>(exact)
            .map_err(link_error)?;
        let (storage, shape, pointer) = match key.key() {
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                let id = value_layout_id(target, exact)?;
                let layout = layouts
                    .get(&id)
                    .and_then(ExactLayoutExportV1::value_handle)
                    .ok_or_else(|| LinkDataError(format!("missing value layout {id}")))?;
                let pointer = match layout.representation().kind() {
                    ExactRepresentationKindV1::QualifiedPointer(kind) => Some(kind),
                    _ => None,
                };
                let result = LinkValueStorage {
                    value: layout.value().clone(),
                    shape: layout.canonical_storage().shape(),
                    pointer,
                };
                self.values.insert(exact, result.clone());
                return Ok(result);
            }
            ExactTypeKey::Tuple(elements) => {
                let values = elements
                    .as_slice()
                    .iter()
                    .map(|exact| self.read(*exact, target, identities, layouts))
                    .collect::<Result<Vec<_>, _>>()?;
                let inputs = values.iter().map(|value| &value.value).collect::<Vec<_>>();
                let tuple =
                    TupleStorageLayoutV1::replay(target, &key, &inputs).map_err(link_error)?;
                (tuple.storage().clone(), ScoopAbiValueShape::Aggregate, None)
            }
            ExactTypeKey::Function { .. } => pointer_storage(
                target.managed_pointer_layout(),
                RefScan::References(vec![0]),
                NichePointerKind::Managed,
            )?,
            ExactTypeKey::RawPointer(_) => pointer_storage(
                target.data_pointer().layout(),
                RefScan::None,
                NichePointerKind::Raw,
            )?,
            ExactTypeKey::NativeFunctionPointer { .. } => pointer_storage(
                target.code_pointer().layout(),
                RefScan::None,
                NichePointerKind::Code,
            )?,
        };
        let value = ValueLayoutConstituentV1::new(
            target,
            LayoutKey::new(exact, target.wire_id(), RepresentationRole::ManagedValue),
            storage,
        )
        .map_err(link_error)?;
        let result = LinkValueStorage {
            value,
            shape,
            pointer,
        };
        self.values.insert(exact, result.clone());
        Ok(result)
    }
}

pub(crate) fn value_layout_id(
    target: LirTargetProfile,
    exact: PersistentExactTypeId,
) -> Result<PersistentLayoutId, LinkDataError> {
    PersistentLayoutId::from_key(&LayoutKey::new(
        exact,
        target.wire_id(),
        RepresentationRole::ManagedValue,
    ))
    .map_err(link_error)
}

pub(crate) fn value_dependencies(
    exact: PersistentExactTypeId,
    target: LirTargetProfile,
    identities: &ValidatedIdentityGraph,
    dependencies: &mut Vec<PersistentLayoutId>,
) -> Result<(), LinkDataError> {
    let key = identities
        .canonical_key::<_, ExactTypeKey>(exact)
        .map_err(link_error)?;
    match key.as_ref() {
        ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
            dependencies.push(value_layout_id(target, exact)?);
        }
        ExactTypeKey::Tuple(elements) => {
            for exact in elements.as_slice() {
                value_dependencies(*exact, target, identities, dependencies)?;
            }
        }
        ExactTypeKey::Function { .. }
        | ExactTypeKey::RawPointer(_)
        | ExactTypeKey::NativeFunctionPointer { .. } => {}
    }
    Ok(())
}

fn pointer_storage(
    layout: ScalarLayout,
    scan: RefScan,
    pointer: NichePointerKind,
) -> Result<
    (
        ValueStorageLayoutV1,
        ScoopAbiValueShape,
        Option<NichePointerKind>,
    ),
    LinkDataError,
> {
    Ok((
        ValueStorageLayoutV1::inline(layout.size_bytes(), layout.alignment_bytes(), scan)
            .map_err(StorageReplayError::Shape)
            .map_err(link_error)?,
        ScoopAbiValueShape::Scalar,
        Some(pointer),
    ))
}

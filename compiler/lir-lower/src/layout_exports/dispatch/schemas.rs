use super::*;

pub(super) enum Schema<'a> {
    Source(&'a mir::ParamFreeMirDispatchSchemaV1),
    Empty,
}

impl Schema<'_> {
    pub fn vtable(&self) -> &[mir::MirDispatchEntryV1] {
        match self {
            Self::Source(source) => source.vtable(),
            Self::Empty => &[],
        }
    }

    pub fn interface(&self, exact: PersistentExactTypeId) -> Option<&[mir::MirDispatchEntryV1]> {
        match self {
            Self::Source(source) => source.interface_table(exact).map(|table| table.entries()),
            Self::Empty => None,
        }
    }
}

pub(super) fn for_owner<'a>(
    bridge: &'a mir::MirTypeBridgeExportConstituentsV1,
    exact: PersistentExactTypeId,
) -> Result<Schema<'a>, Error> {
    let ty = bridge.types().get(exact).ok_or(Error::MissingType(exact))?;
    match ty.representation() {
        mir::MirTypeRepresentationV1::Struct { .. }
        | mir::MirTypeRepresentationV1::Enum { .. }
        | mir::MirTypeRepresentationV1::CoroutineStep { .. }
        | mir::MirTypeRepresentationV1::CoroutineSlot { .. } => Ok(Schema::Empty),
        mir::MirTypeRepresentationV1::Intrinsic(intrinsic)
            if !matches!(intrinsic, mir::MirParamFreeIntrinsicV1::String) =>
        {
            Ok(Schema::Empty)
        }
        mir::MirTypeRepresentationV1::BoxedValue { .. }
            if ty.base_and_interfaces().base == mir::MirBaseClassV1::None
                && ty.base_and_interfaces().interfaces.is_empty() =>
        {
            Ok(Schema::Empty)
        }
        mir::MirTypeRepresentationV1::BoxedValue { payload } => {
            if let Some(schema) = bridge.dispatch().get(exact) {
                Ok(Schema::Source(schema))
            } else {
                source(bridge, payload.value)
            }
        }
        _ => source(bridge, exact),
    }
}

fn source(
    bridge: &mir::MirTypeBridgeExportConstituentsV1,
    exact: PersistentExactTypeId,
) -> Result<Schema<'_>, Error> {
    bridge
        .dispatch()
        .get(exact)
        .map(Schema::Source)
        .ok_or(Error::MissingDispatch(exact))
}

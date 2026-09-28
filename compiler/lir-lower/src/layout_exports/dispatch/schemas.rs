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
        mir::MirTypeRepresentationV1::BoxedValue { payload } => for_owner(bridge, payload.value),
        mir::MirTypeRepresentationV1::Struct { .. }
        | mir::MirTypeRepresentationV1::Enum { .. }
        | mir::MirTypeRepresentationV1::InlineArray { .. }
        | mir::MirTypeRepresentationV1::CoroutineStep { .. }
        | mir::MirTypeRepresentationV1::CoroutineSlot { .. }
        | mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::Unit)
            if ty.base_and_interfaces().base == mir::MirBaseClassV1::None
                && ty.base_and_interfaces().interfaces.is_empty() =>
        {
            Ok(Schema::Empty)
        }
        _ => bridge
            .dispatch()
            .get(exact)
            .map(Schema::Source)
            .ok_or(Error::MissingDispatch(exact)),
    }
}

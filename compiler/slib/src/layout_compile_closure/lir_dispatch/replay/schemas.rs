use super::*;

pub(super) enum Schema<'a> {
    Empty,
    Source(&'a mir::ParamFreeMirDispatchSchemaV1),
}
impl Schema<'_> {
    pub(super) fn vtable(&self) -> &[mir::MirDispatchEntryV1] {
        match self {
            Self::Empty => &[],
            Self::Source(source) => source.vtable().entries(),
        }
    }
    pub(super) fn itables(&self) -> &[mir::MirInterfaceDispatchTableV1] {
        match self {
            Self::Empty => &[],
            Self::Source(source) => source.itables(),
        }
    }
}

pub(super) fn for_owner<'a>(
    types: &'a mir::CanonicalParamFreeMirTypeExportsV1,
    schemas: &'a mir::CanonicalMirDispatchSchemasV1,
    ty: &'a mir::ParamFreeMirTypeExportV1,
    meter: &mut BudgetMeter,
) -> Result<Schema<'a>, Error> {
    use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Representation};
    match ty.representation() {
        Representation::Class { .. }
        | Representation::Object { .. }
        | Representation::Intrinsic(Intrinsic::String)
        | Representation::BoxedValue { .. } => source(types, schemas, ty, meter, 1),
        Representation::Intrinsic(_)
        | Representation::Struct { .. }
        | Representation::Enum { .. }
        | Representation::Interface
        | Representation::ObjectBacking { .. }
        | Representation::CoroutineStep { .. }
        | Representation::CoroutineSlot { .. } => Ok(Schema::Empty),
    }
}

fn source<'a>(
    types: &'a mir::CanonicalParamFreeMirTypeExportsV1,
    schemas: &'a mir::CanonicalMirDispatchSchemasV1,
    ty: &'a mir::ParamFreeMirTypeExportV1,
    meter: &mut BudgetMeter,
    depth: u64,
) -> Result<Schema<'a>, Error> {
    let path = WirePath::root();
    meter.check_semantic_depth(depth, &path)?;
    meter.charge_work(1, &path)?;
    match ty.representation() {
        mir::MirTypeRepresentationV1::BoxedValue { payload } => {
            meter.charge_edges(1, &path)?;
            meter.charge_work(u64::from(types.records().len().max(1).ilog2()) + 1, &path)?;
            let payload = types
                .get(payload.value)
                .ok_or(Error::MissingType(payload.value))?;
            source(types, schemas, payload, meter, depth + 1)
        }
        mir::MirTypeRepresentationV1::Intrinsic(mir::MirParamFreeIntrinsicV1::Unit)
        | mir::MirTypeRepresentationV1::CoroutineStep { .. }
        | mir::MirTypeRepresentationV1::CoroutineSlot { .. } => {
            if ty.base_and_interfaces().base != mir::MirBaseClassV1::None
                || !ty.base_and_interfaces().interfaces.is_empty()
            {
                return Err(Error::FiniteInheritance(ty.exact()));
            }
            Ok(Schema::Empty)
        }
        _ => {
            meter.charge_work(u64::from(schemas.records().len().max(1).ilog2()) + 1, &path)?;
            schemas
                .get(ty.exact())
                .map(Schema::Source)
                .ok_or(Error::MissingSchema(ty.exact()))
        }
    }
}

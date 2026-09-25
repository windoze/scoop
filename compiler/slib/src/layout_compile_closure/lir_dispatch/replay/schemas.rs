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
) -> Result<Schema<'a>, Error> {
    use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Representation};
    match ty.representation() {
        Representation::Class { .. }
        | Representation::Object { .. }
        | Representation::Intrinsic(Intrinsic::String)
        | Representation::BoxedValue { .. } => source(types, schemas, ty),
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
) -> Result<Schema<'a>, Error> {
    match ty.representation() {
        mir::MirTypeRepresentationV1::BoxedValue { payload } => {
            let payload = types
                .get(payload.value)
                .ok_or(Error::MissingType(payload.value))?;
            source(types, schemas, payload)
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
        _ => schemas
            .get(ty.exact())
            .map(Schema::Source)
            .ok_or(Error::MissingSchema(ty.exact())),
    }
}

use super::*;
use crate::{Module, Type};

type Projection = (
    MirTypeFactsV1,
    MirTypeRepresentationV1,
    MirBaseAndInterfacesV1,
);

pub(super) fn project(
    input: &SingleConeStrongMirInput,
    location: GeneratedExactTypeLocation,
    role: &GeneratedNominalKey,
    meter: &mut BudgetMeter,
) -> Result<Projection, MirTypeBridgeError> {
    let module = input.module();
    let mut bases = MirBaseAndInterfacesV1 {
        base: MirBaseClassV1::None,
        interfaces: Vec::new(),
    };
    let (facts, representation) = match (location, role) {
        (GeneratedExactTypeLocation::Class(class), GeneratedNominalKey::BoxedValue { .. }) => {
            meter
                .charge_work(module.meta.boxed_types.len() as u64, &WirePath::root())
                .map_err(MirTypeBridgeError::Resource)?;
            let boxed = module
                .meta
                .boxed_types
                .iter()
                .find(|boxed| boxed.class() == class)
                .expect("validated MIR identifies every finite value box");
            let fields = module.classes[class].declared_fields();
            let [payload] = fields else {
                unreachable!("validated boxes have exactly one payload field")
            };
            let interfaces = &module.classes[class].interfaces;
            reserve(&mut bases.interfaces, interfaces.len(), meter)?;
            for interface in interfaces {
                bases
                    .interfaces
                    .push(exact(module, &Type::Interface(*interface), meter)?);
            }
            charge_sort(bases.interfaces.len(), meter)?;
            bases.interfaces.sort_unstable();
            bases.interfaces.dedup();
            (
                MirTypeFactsV1::try_new(
                    MirValueKindV1::Reference,
                    MirGcKindV1::ContainsManagedReferences,
                )?,
                MirTypeRepresentationV1::BoxedValue {
                    payload: MirRepresentationFieldV1 {
                        field: boxed.identity().payload_field_record().id(),
                        value: exact(module, &payload.ty, meter)?,
                    },
                },
            )
        }
        (
            GeneratedExactTypeLocation::Enum(id),
            GeneratedNominalKey::CoroutineStep { .. } | GeneratedNominalKey::CoroutineSlot { .. },
        ) => {
            let definition = &module.enums[id];
            let variants = variants(module, &definition.variants, meter)?;
            let representation = if matches!(role, GeneratedNominalKey::CoroutineStep { .. }) {
                MirTypeRepresentationV1::CoroutineStep { variants }
            } else {
                MirTypeRepresentationV1::CoroutineSlot { variants }
            };
            (
                MirTypeFactsV1::try_new(MirValueKindV1::NonZeroValue, gc(definition.gc_free))?,
                representation,
            )
        }
        _ => unreachable!("the finite producer selects only sealed box, step and slot roots"),
    };
    Ok((facts, representation, bases))
}

fn variants(
    module: &Module,
    definitions: &[crate::VariantDef],
    meter: &mut BudgetMeter,
) -> Result<Vec<MirRepresentationVariantV1>, MirTypeBridgeError> {
    let path = WirePath::root();
    let mut variants = Vec::new();
    reserve(&mut variants, definitions.len(), meter)?;
    for variant in definitions {
        meter
            .charge_nodes(1, &path)
            .map_err(MirTypeBridgeError::Resource)?;
        let mut fields = Vec::new();
        reserve(&mut fields, variant.fields.len(), meter)?;
        for field in &variant.fields {
            fields.push(MirRepresentationVariantFieldV1 {
                field: field.identity,
                value: exact(module, &field.ty, meter)?,
            });
        }
        variants.push(MirRepresentationVariantV1 {
            variant: variant.identity,
            fields,
            gc: gc(variant.gc_free),
        });
    }
    Ok(variants)
}

fn exact(
    module: &Module,
    ty: &Type,
    meter: &mut BudgetMeter,
) -> Result<PersistentExactTypeId, MirTypeBridgeError> {
    meter
        .charge_work(
            module.meta.source_exact_types.len() as u64,
            &WirePath::root(),
        )
        .map_err(MirTypeBridgeError::Resource)?;
    Ok(module
        .meta
        .source_exact_types
        .get(ty)
        .expect("finite helper payloads and conformance types retain source exact identities")
        .identity_record()
        .id())
}

fn gc(gc_free: bool) -> MirGcKindV1 {
    if gc_free {
        MirGcKindV1::GcFree
    } else {
        MirGcKindV1::ContainsManagedReferences
    }
}

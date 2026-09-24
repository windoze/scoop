use super::*;
use mir::MirTypeRepresentationV1 as Repr;

pub(super) fn check(replay: &Replay<'_, '_>, combined: bool) {
    structs(replay);
    classes(replay);
    helpers(replay);
    if combined {
        variants(replay);
        object_backing(replay);
    }
}

fn structs(replay: &Replay<'_, '_>) {
    let record = replay.section.types().records().iter().find(|record| {
        record.facts().kind() == mir::MirValueKindV1::NonZeroValue
            && matches!(record.representation(), Repr::Struct { fields, .. } if fields.len() > 1)
    }).unwrap();
    let exact = record.exact();
    let mut shape = record.representation().clone();
    let Repr::Struct { fields, .. } = &mut shape else {
        unreachable!()
    };
    fields.swap(0, 1);
    component(
        replay.reject_representation(record, shape),
        Component::Field { index: 0 },
        exact,
    );

    let mut shape = record.representation().clone();
    let Repr::Struct { fields, .. } = &mut shape else {
        unreachable!()
    };
    fields.pop().unwrap();
    component(
        replay.reject_representation(record, shape),
        Component::FieldCount,
        exact,
    );

    let mut shape = record.representation().clone();
    let Repr::Struct { fields, .. } = &mut shape else {
        unreachable!()
    };
    fields[0].value = replay
        .section
        .types()
        .records()
        .iter()
        .find(|candidate| candidate.exact() != fields[0].value)
        .unwrap()
        .exact();
    component(
        replay.reject_representation(record, shape),
        Component::Field { index: 0 },
        exact,
    );

    let mut shape = record.representation().clone();
    let Repr::Struct {
        interior_mutable, ..
    } = &mut shape
    else {
        unreachable!()
    };
    *interior_mutable = !*interior_mutable;
    component(
        replay.reject_representation(record, shape),
        Component::InteriorMutable,
        exact,
    );

    let mut shape = record.representation().clone();
    let Repr::Struct { c_layout, .. } = &mut shape else {
        unreachable!()
    };
    *c_layout = match c_layout {
        mir::MirTypeCLayoutPolicyV1::Ordinary => {
            mir::MirTypeCLayoutPolicyV1::CLayout(mir::MirCLayoutContract {
                aligned: mir::MirCLayoutValue::A8,
                packed: mir::MirCLayoutValue::A1,
            })
        }
        mir::MirTypeCLayoutPolicyV1::CLayout(_) => mir::MirTypeCLayoutPolicyV1::Ordinary,
    };
    component(
        replay.reject_representation(record, shape),
        Component::CLayout,
        exact,
    );

    let gc = match record.facts().gc() {
        mir::MirGcKindV1::GcFree => mir::MirGcKindV1::ContainsManagedReferences,
        mir::MirGcKindV1::ContainsManagedReferences => mir::MirGcKindV1::GcFree,
    };
    let facts = mir::MirTypeFactsV1::try_new(record.facts().kind(), gc).unwrap();
    component(
        replay.reject(replay.replace(
            record,
            record.representation().clone(),
            facts,
            record.base_and_interfaces().clone(),
        )),
        Component::Facts,
        exact,
    );
}

fn classes(replay: &Replay<'_, '_>) {
    let record = replay
        .section
        .types()
        .records()
        .iter()
        .find(|record| {
            matches!(record.representation(), Repr::Class { .. })
                && replay
                    .source
                    .representations()
                    .get(record.origin().nominal())
                    .is_some()
        })
        .unwrap();
    let mut shape = record.representation().clone();
    let Repr::Class { kind, .. } = &mut shape else {
        unreachable!()
    };
    *kind = if *kind == mir::MirClassKindV1::Abstract {
        mir::MirClassKindV1::Final
    } else {
        mir::MirClassKindV1::Abstract
    };
    component(
        replay.reject_representation(record, shape),
        Component::ClassKind,
        record.exact(),
    );

    if let Some(record) = replay.section.types().records().iter().find(|record| {
        matches!(record.origin(), mir::MirTypeOriginV1::SourceNominal(_))
            && record.base_and_interfaces().base != mir::MirBaseClassV1::None
    }) {
        let mut bases = record.base_and_interfaces().clone();
        bases.base = mir::MirBaseClassV1::None;
        component(
            replay.reject(replay.replace(
                record,
                record.representation().clone(),
                record.facts(),
                bases,
            )),
            Component::Base,
            record.exact(),
        );
    }
    if let Some(record) = replay.section.types().records().iter().find(|record| {
        matches!(record.origin(), mir::MirTypeOriginV1::SourceNominal(_))
            && !record.base_and_interfaces().interfaces.is_empty()
    }) {
        let mut bases = record.base_and_interfaces().clone();
        bases.interfaces.clear();
        component(
            replay.reject(replay.replace(
                record,
                record.representation().clone(),
                record.facts(),
                bases,
            )),
            Component::Interfaces,
            record.exact(),
        );
    }
}

fn helpers(replay: &Replay<'_, '_>) {
    let interface = replay
        .section
        .types()
        .records()
        .iter()
        .find(|record| matches!(record.representation(), Repr::Interface))
        .unwrap()
        .exact();
    let helper = replay
        .section
        .types()
        .records()
        .iter()
        .find(|record| matches!(record.representation(), Repr::CoroutineStep { .. }))
        .unwrap();
    let mut bases = helper.base_and_interfaces().clone();
    bases.interfaces.push(interface);
    component(
        replay.reject(replay.replace(
            helper,
            helper.representation().clone(),
            helper.facts(),
            bases,
        )),
        Component::Interfaces,
        helper.exact(),
    );
}

fn variants(replay: &Replay<'_, '_>) {
    let record = replay
        .section
        .types()
        .records()
        .iter()
        .find(|record| {
            matches!(record.representation(), Repr::Enum { variants }
            if variants.len() > 1 && variants.iter().any(|variant| !variant.fields.is_empty()))
        })
        .unwrap();
    let mut shape = record.representation().clone();
    let Repr::Enum { variants } = &mut shape else {
        unreachable!()
    };
    variants.swap(0, 1);
    component(
        replay.reject_representation(record, shape),
        Component::Variant { index: 0 },
        record.exact(),
    );

    let mut shape = record.representation().clone();
    let Repr::Enum { variants } = &mut shape else {
        unreachable!()
    };
    let (variant, fields) = variants
        .iter_mut()
        .enumerate()
        .find_map(|(index, variant)| {
            (!variant.fields.is_empty()).then_some((index, &mut variant.fields))
        })
        .unwrap();
    fields[0].value = replay
        .section
        .types()
        .records()
        .iter()
        .find(|candidate| candidate.exact() != fields[0].value)
        .unwrap()
        .exact();
    component(
        replay.reject_representation(record, shape),
        Component::VariantField { variant, index: 0 },
        record.exact(),
    );
}

fn object_backing(replay: &Replay<'_, '_>) {
    let record = replay.section.types().records().iter().find(|record| {
        matches!(record.representation(), Repr::ObjectBacking { declared_fields } if !declared_fields.is_empty())
    }).unwrap();
    component(
        replay.reject_representation(
            record,
            Repr::ObjectBacking {
                declared_fields: vec![],
            },
        ),
        Component::FieldCount,
        record.exact(),
    );
}

//! Complete base-instance prefixes, independent of runtime allocation.

use super::*;

#[test]
fn derived_fields_start_after_the_complete_base_instance() {
    let mut b = Builder::new();
    let byte = mir::Type::Integer(mir::IntegerKind::SIGNED_8);
    let base = b.class(
        "PrefixBase",
        None,
        &[("byte", byte.clone())],
        vec![],
        vec![],
    );
    b.classes[base].modifier = mir::ClassModifier::Abstract;
    let derived = b.class(
        "PrefixDerived",
        Some(base),
        &[("byte", byte.clone()), ("own", byte.clone())],
        vec![],
        vec![],
    );
    b.class(
        "PrefixLeaf",
        Some(derived),
        &[
            ("byte", byte.clone()),
            ("own", byte),
            ("ref", mir::Type::String),
        ],
        vec![],
        vec![],
    );
    let main = b.main(Arena::new(), vec![]);
    let module = lower(b.finish(main));
    let projection = ["PrefixBase", "PrefixDerived", "PrefixLeaf"]
        .into_iter()
        .map(|name| {
            let layout = layout_values(&module)
                .find(|layout| layout.name == name)
                .unwrap();
            let offsets: Vec<_> = layout.fields.iter().map(|field| field.offset).collect();
            format!(
                "{name}: size={} align={} offsets={offsets:?} refs={:?}",
                layout.size,
                layout.align,
                plain_refs(layout)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(projection, @"
    PrefixBase: size=24 align=8 offsets=[16] refs=[]
    PrefixDerived: size=32 align=8 offsets=[16, 24] refs=[]
    PrefixLeaf: size=40 align=8 offsets=[16, 24, 32] refs=[32]
    ");
}

#[test]
fn own_zst_fields_do_not_consume_base_padding_or_extend_the_prefix() {
    let mut b = Builder::new();
    let byte = mir::Type::Integer(mir::IntegerKind::SIGNED_8);
    let base = b.class(
        "ZstPrefixBase",
        None,
        &[("byte", byte.clone())],
        vec![],
        vec![],
    );
    let empty = b.strukt("EmptyPrefixField", &[]);
    b.class(
        "ZstPrefixDerived",
        Some(base),
        &[
            ("byte", byte.clone()),
            ("empty", mir::Type::Struct(empty)),
            ("own", byte),
        ],
        vec![],
        vec![],
    );
    let main = b.main(Arena::new(), vec![]);
    let module = lower(b.finish(main));
    let layout = layout_values(&module)
        .find(|layout| layout.name == "ZstPrefixDerived")
        .unwrap();
    assert_eq!((layout.size, layout.align), (32, 8));
    assert_eq!(
        layout
            .fields
            .iter()
            .map(|field| field.offset)
            .collect::<Vec<_>>(),
        [16, 0, 24]
    );
    assert_eq!(plain_refs(layout), []);
}

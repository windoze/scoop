use super::*;
use scoop_identity::{DeclarationName, SourceDeclarationKey};

pub(super) fn check(
    name: &str,
    input: LayoutAbiExportInputV1<'_>,
    layouts: &lir::CanonicalExactLayoutExportsV1,
) {
    let mut rows = Vec::new();
    for source in input.bridge.types().records() {
        let mir::MirTypeOriginV1::SourceNominal(nominal) = source.origin() else {
            continue;
        };
        let declaration = input
            .identities
            .canonical_key::<_, SourceDeclarationKey>(*nominal)
            .unwrap();
        let DeclarationName::Named(name) = declaration.name() else {
            panic!("source nominal name");
        };
        if !name.as_str().starts_with("SharedLayout") {
            continue;
        }
        for layout in layouts
            .records()
            .iter()
            .filter(|layout| layout.identity().exact() == source.exact())
        {
            rows.push(format!(
                "{} {:?}: {}\n",
                name.as_str(),
                layout.identity().layout_key().representation(),
                body(layout),
            ));
        }
    }
    rows.sort();
    assert!(!rows.is_empty());
    let dump = rows.concat();
    let snapshot = crate::workspace_root().join(format!(
        "tests/fixtures/m23-core-layout-exports/{name}.layouts.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_CORE_LAYOUT_EXPORTS").is_some() {
        std::fs::write(&snapshot, &dump).unwrap();
    }
    assert_eq!(dump, std::fs::read_to_string(snapshot).unwrap());
}

fn body(layout: &lir::ExactLayoutExportV1) -> String {
    match layout.kind() {
        lir::ExactLayoutBodyKindV1::Value(value) => {
            let storage = value.value().storage();
            let details = match value.representation().kind() {
                lir::ExactRepresentationKindV1::Struct(structure) => format!(
                    "struct mutable={} fields={:?}",
                    structure.interior_mutable(),
                    fields(structure.fields()),
                ),
                lir::ExactRepresentationKindV1::TaggedEnum(enumeration) => format!(
                    "enum variants={:?}",
                    enumeration
                        .variants()
                        .iter()
                        .map(|variant| variant
                            .fields()
                            .iter()
                            .map(|field| (
                                field.storage().offset().get(),
                                field.access_alignment().get()
                            ))
                            .collect::<Vec<_>>())
                        .collect::<Vec<_>>(),
                ),
                kind => format!("{kind:?}"),
            };
            format!(
                "value size={} align={} scan={:?} {details}",
                storage.byte_size(),
                storage.alignment().get(),
                storage
                    .nonzero()
                    .map(|storage| storage.scan().as_ref_scan()),
            )
        }
        lir::ExactLayoutBodyKindV1::Instance(instance) => {
            let shape = instance.shape();
            let details = match instance.representation().kind() {
                lir::InstanceRepresentationKindV1::ClassObject(class) => {
                    let prefix = match class.base_prefix() {
                        lir::ClassBasePrefixV1::NoBase => 0,
                        lir::ClassBasePrefixV1::BasePrefix { byte_size, .. } => byte_size,
                    };
                    format!(
                        "class prefix={prefix} fields={:?}",
                        fields(class.complete_fields())
                    )
                }
                lir::InstanceRepresentationKindV1::BoxedPayload(payload) => {
                    format!("box payload-size={}", payload.storage().byte_size())
                }
                kind => format!("{kind:?}"),
            };
            format!(
                "instance {:?} size={} align={} scan={:?} inline={:?} {details}",
                shape.instance_kind(),
                shape.minimum_size(),
                shape.instance_alignment(),
                shape.object_scan(),
                shape.inline_scan(),
            )
        }
    }
}

fn fields(fields: &[lir::PlacedFieldStorageV1]) -> Vec<(u64, u64)> {
    fields
        .iter()
        .map(|field| {
            (
                field.storage().offset().get(),
                field.access_alignment().get(),
            )
        })
        .collect()
}

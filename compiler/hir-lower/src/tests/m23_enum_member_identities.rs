use super::*;
use std::fmt::Write;

fn project(source: &str) -> String {
    let output = lower(&[complete_core_file(), scoop_parser::parse(source).unwrap()]).unwrap();
    let mir = scoop_mir_lower::lower(&output.local).unwrap();
    let export = output.export.module();
    let concrete = output.local.module();
    let mut rows = Vec::new();
    for (source_id, declaration) in export.enums.iter() {
        if !["Signal", "Envelope", "Bundle"].contains(&declaration.name.as_str()) {
            continue;
        }
        for (_, local) in concrete
            .enums
            .iter()
            .filter(|(_, local)| local.origin == export.nominal_identities[source_id])
        {
            let exact = concrete.exact_type_identities[local.canonical_type].id();
            let identity = mir.meta.source_exact_types.get_by_identity(exact).unwrap();
            let scoop_mir::Type::Enum(id, _) = identity.ty() else {
                panic!("an enum exact identity retains an enum MIR type")
            };
            let lowered = &mir.enums[*id];
            assert_eq!(local.variants.len(), declaration.variants.len());
            assert_eq!(lowered.variants.len(), declaration.variants.len());
            let mut row = format!("{} exact:{exact}\n", declaration.name);
            for (index, ((source, local), lowered)) in declaration
                .variants
                .iter()
                .zip(&local.variants)
                .zip(&lowered.variants)
                .enumerate()
            {
                let reference = hir::EnumVariantRef::checked(
                    &export.enums,
                    source_id,
                    u32::try_from(index).unwrap(),
                )
                .unwrap();
                let expected = export.enum_member_identities[reference].id();
                assert_eq!(local.identity, expected);
                assert_eq!(lowered.identity, expected);
                assert_eq!(local.fields.len(), source.fields.len());
                assert_eq!(lowered.fields.len(), source.fields.len());
                writeln!(row, "  {} variant:{expected}", source.name).unwrap();
                for (field_index, (local_field, mir_field)) in
                    local.fields.iter().zip(&lowered.fields).enumerate()
                {
                    let field = hir::EnumVariantFieldRef::checked(
                        &export.enums,
                        reference,
                        u32::try_from(field_index).unwrap(),
                    )
                    .unwrap();
                    let expected_field = export.enum_member_identities[field].id();
                    assert_eq!(local_field.identity, expected_field);
                    assert_eq!(mir_field.identity, expected_field);
                    let expected_type = concrete.exact_type_identities[local_field.ty].id();
                    assert_eq!(
                        mir.meta
                            .source_exact_types
                            .get(&mir_field.ty)
                            .unwrap()
                            .identity_record()
                            .id(),
                        expected_type
                    );
                    let variant = scoop_mir::MirVariantRef::new(
                        &mir.enums,
                        *id,
                        u32::try_from(index).unwrap(),
                    )
                    .unwrap();
                    let field = scoop_mir::MirVariantFieldRef::new(
                        &mir.enums,
                        variant,
                        u32::try_from(field_index).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(
                        field.definition(&mir.enums).unwrap().identity,
                        expected_field
                    );
                    writeln!(
                        row,
                        "    {field_index} field:{expected_field}: exact:{expected_type}"
                    )
                    .unwrap();
                }
            }
            rows.push(row);
        }
    }
    assert!(!rows.is_empty());
    rows.sort();
    rows.concat()
}

#[test]
fn enum_members_preserve_source_identity_through_concrete_hir_and_mir() {
    for fixture in ["plain", "combined"] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-enum-member-identities");
        let source = std::fs::read_to_string(root.join(format!("{fixture}.scoop"))).unwrap();
        let projected = project(&source);
        assert_eq!(
            projected,
            project(&format!("enum Unrelated {{ Added }}\n{source}"))
        );
        if let Some(output) = std::env::var_os("SCOOP_ENUM_MEMBER_SNAPSHOT_DIR") {
            std::fs::create_dir_all(&output).unwrap();
            std::fs::write(
                std::path::Path::new(&output).join(format!("{fixture}.snap")),
                &projected,
            )
            .unwrap();
        } else {
            assert_eq!(
                projected,
                std::fs::read_to_string(root.join(format!("{fixture}.snap"))).unwrap()
            );
        }
    }
}

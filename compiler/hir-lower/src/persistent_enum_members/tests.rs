use std::collections::BTreeMap;

use scoop_ast as ast;
use scoop_hir as hir;
use scoop_identity::{
    EnumVariantFieldSelector, PersistentEnumVariantFieldId, PersistentEnumVariantId,
};

use crate::tests::{
    core_file, core_source_identity, enum_decl, file, test_source_identity, ty_named,
    variant_named, variant_positional, variant_unit,
};

#[derive(Debug, Eq, PartialEq)]
struct EnumMemberIds {
    variants: BTreeMap<String, PersistentEnumVariantId>,
    fields: BTreeMap<(String, String), PersistentEnumVariantFieldId>,
    selectors: BTreeMap<(String, String), EnumVariantFieldSelector>,
    styles: BTreeMap<String, hir::VariantStyle>,
}

fn identities(variants: Vec<ast::VariantDecl>) -> EnumMemberIds {
    let source = file(vec![enum_decl("Choice", Vec::new(), variants)]);
    let parsed = ast::AllParsedSources::try_new(ast::NonEmptyVec::new(
        ast::IdentifiedParsedSource::new(test_source_identity("src/choice.scoop"), source),
        Vec::new(),
    ))
    .unwrap();
    let core = core_file();
    let input = crate::LegacyCombinedSources::try_new(
        vec![crate::ProviderSource {
            source: &core,
            identity: core_source_identity("src/core.scoop"),
            provider: hir::IntrinsicProviderId::from_raw(0),
            name: "core",
            source_text: "",
        }],
        hir::IntrinsicProviderId::from_raw(1),
        parsed,
        |_| crate::CurrentSourceDetails {
            display_locator: "/display/choice.scoop",
            source_text: "",
        },
    )
    .unwrap();
    let output =
        crate::lower_legacy_combined_sources(&input, crate::IntrinsicDeclarationPolicy::CoreOnly)
            .expect("the enum-member fixture lowers");
    let (enum_id, enumeration) = output
        .export
        .enums
        .iter()
        .find(|(_, enumeration)| enumeration.name == "Choice")
        .unwrap();
    let mut result = EnumMemberIds {
        variants: BTreeMap::new(),
        fields: BTreeMap::new(),
        selectors: BTreeMap::new(),
        styles: BTreeMap::new(),
    };
    for (variant_index, variant) in enumeration.variants.iter().enumerate() {
        let variant_ref = hir::EnumVariantRef::checked(
            &output.export.enums,
            enum_id,
            u32::try_from(variant_index).unwrap(),
        )
        .unwrap();
        result.variants.insert(
            variant.name.clone(),
            output.export.enum_member_identities[variant_ref].id(),
        );
        result.styles.insert(variant.name.clone(), variant.style);
        for (field_index, field) in variant.fields.iter().enumerate() {
            let field_ref = hir::EnumVariantFieldRef::checked(
                &output.export.enums,
                variant_ref,
                u32::try_from(field_index).unwrap(),
            )
            .unwrap();
            let record = &output.export.enum_member_identities[field_ref];
            let key = (variant.name.clone(), field.name.clone());
            result.fields.insert(key.clone(), record.id());
            result
                .selectors
                .insert(key, record.key().selector().clone());
        }
    }
    result
}

#[test]
fn named_member_identities_ignore_order_and_field_types() {
    let first = identities(vec![
        variant_named(
            "Value",
            vec![("left", ty_named("Int")), ("right", ty_named("String"))],
        ),
        variant_unit("Empty"),
    ]);
    let reordered = identities(vec![
        variant_unit("Empty"),
        variant_named(
            "Value",
            vec![("right", ty_named("Int")), ("left", ty_named("String"))],
        ),
    ]);

    assert_eq!(first, reordered);
    assert_eq!(first.styles["Value"], hir::VariantStyle::Named);
    assert_eq!(first.styles["Empty"], hir::VariantStyle::Unit);
}

#[test]
fn positional_selector_is_not_inferred_from_the_display_field_name() {
    let positional = identities(vec![variant_positional("Value", vec![ty_named("Int")])]);
    let named = identities(vec![variant_named(
        "Value",
        vec![("_1", ty_named("String"))],
    )]);
    let key = ("Value".to_string(), "_1".to_string());

    assert_eq!(
        positional.selectors[&key],
        EnumVariantFieldSelector::Positional {
            declaration_index: 0,
        }
    );
    assert!(matches!(
        &named.selectors[&key],
        EnumVariantFieldSelector::Named(name) if name.as_str() == "_1"
    ));
    assert_ne!(positional.fields[&key], named.fields[&key]);
}

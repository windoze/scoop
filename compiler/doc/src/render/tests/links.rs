use super::*;

fn links(inputs: &[ConeInput], cone: &str) -> serde_json::Value {
    let catalog = SiteCatalog::new(inputs, SiteOptions::default()).unwrap();
    serde_json::to_value(catalog.link_inputs(&ConeKey(cone.into())).unwrap()).unwrap()
}

#[test]
fn link_inputs_follow_availability_but_ignore_unrelated_content_changes() {
    let shape = item("shape", "Shape", ApiKind::Class, None);
    let mut consumer = item("consumer", "useShape", ApiKind::Function, None);
    consumer.signature.push(SignatureToken {
        text: "Shape".into(),
        link: Some(shape.key.clone()),
    });
    let mut docs = comment("[Shape]");
    docs.body.references.push(SymbolReference {
        range: TextRange { start: 0, end: 7 },
        target: LinkTarget::Symbols(vec![shape.key.clone()]),
    });
    consumer.comment = Some(docs);
    let mut inputs = vec![
        cone("provider", vec![shape]),
        cone("consumer", vec![consumer]),
        cone("other", vec![]),
    ];
    let baseline = links(&inputs, "consumer");
    inputs[0].items[0].comment = Some(comment("New implementation notes."));
    inputs[2]
        .items
        .push(item("unrelated", "Unrelated", ApiKind::Class, None));
    assert_eq!(baseline, links(&inputs, "consumer"));
    inputs[0].items[0].hidden = true;
    assert_ne!(baseline, links(&inputs, "consumer"));
    inputs[0].items[0].hidden = false;
    inputs.push(cone("another", vec![]));
    assert_ne!(baseline, links(&inputs, "consumer"));
}

#[test]
fn link_inputs_include_extension_displays_and_reexport_visibility() {
    let shape = item("shape", "Shape", ApiKind::Class, None);
    let mut extension = item("extension", "extend", ApiKind::Function, None);
    extension.receiver = Some(SignatureToken {
        text: "Shape".into(),
        link: Some(shape.key.clone()),
    });
    let mut inputs = vec![
        cone("provider", vec![shape]),
        cone("extensions", vec![extension]),
        cone("facade", vec![]),
    ];
    inputs[2].bindings.push(ExportBinding {
        package: vec![],
        name: "Alias".into(),
        target: SymbolKey("shape".into()),
        reexport: true,
    });
    let provider = links(&inputs, "provider");
    let facade = links(&inputs, "facade");
    inputs[1].items[0].signature[0].text = "public extend(value: Int)".into();
    assert_ne!(provider, links(&inputs, "provider"));
    assert_eq!(facade, links(&inputs, "facade"));
    inputs[0].items[0].hidden = true;
    let hidden = links(&inputs, "facade");
    assert_ne!(facade, hidden);
    inputs[0].items.clear();
    assert_ne!(hidden, links(&inputs, "facade"));
}

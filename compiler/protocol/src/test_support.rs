use crate::{CurrentConeRequestV1, HostPathCarrier, package::ExpandedConeInputV1};

pub(crate) fn manifest(root: &str) -> CurrentConeRequestV1 {
    let root = std::env::current_dir().unwrap().join(root);
    let text =
        "schema=1\n[cone]\ngroup='org.example'\nname='library'\nversion='1.0.0'\nkind='library'\n";
    CurrentConeRequestV1::ExpandedCone {
        config: Box::new(ExpandedConeInputV1 {
            root: HostPathCarrier::from_path(&root).unwrap(),
            manifest_path: HostPathCarrier::from_path(&root.join("Cone.toml")).unwrap(),
            manifest_text: text.into(),
            manifest: scoop_manifest::parse_cone_manifest(text, &[]).unwrap(),
            metadata: Default::default(),
        }),
    }
}

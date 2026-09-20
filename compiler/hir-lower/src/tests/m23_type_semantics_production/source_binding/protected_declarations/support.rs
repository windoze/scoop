use super::*;

pub(super) fn expected(
    fixture: &Fixture,
    sources: &Sources,
) -> hir::CanonicalProtectedDeclarationRefsV1 {
    use hir::ProtectedDeclarationRefV1 as Ref;
    let protected = hir::DeclaredVisibilityV1::Protected;
    let mut values = Vec::new();
    for r in sources.members.callables.records() {
        if r.declaration_access().declared_visibility() == protected {
            values.push(Ref::Callable(
                hir::ProtectedCallableDeclarationRefV1::try_new(r.declaration()).unwrap(),
            ));
        }
    }
    for r in sources.constructors.records() {
        if r.declaration_access().declared_visibility() == protected {
            values.push(Ref::Constructor(r.declaration()));
        }
    }
    for r in sources.members.properties.records() {
        if r.declaration_access().declared_visibility() == protected {
            values.push(Ref::Property(r.declaration()));
        }
    }
    for r in fixture.source.entries().sources.records() {
        if r.access().declared_visibility() == protected {
            values.push(Ref::NestedNominal(r.owner()));
        }
    }
    hir::CanonicalProtectedDeclarationRefsV1::try_new(values).unwrap()
}

pub(in crate::tests::m23_type_semantics_production::source_binding) fn restore(
    fixture: &mut Fixture,
    produced: &Production,
) -> (
    hir::CanonicalProtectedDeclarationInterfacesV1,
    hir::CanonicalProtectedCallableSourceInterfacesV1,
) {
    let bytes = encode(produced.required()).unwrap();
    let decoded: hir::DecodedCanonicalProtectedDeclarationRefsV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let restored = decoded
        .resolve(&mut fixture.identities, &mut meter())
        .unwrap();
    assert_eq!(encode(&restored).unwrap(), bytes);
    let bytes = encode(produced.declarations()).unwrap();
    let decoded: hir::DecodedCanonicalProtectedDeclarationInterfacesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let declarations = decoded
        .resolve(&mut fixture.identities, &mut meter())
        .unwrap();
    assert_eq!(encode(&declarations).unwrap(), bytes);
    let keys = hir::ProtectedDefaultKeyIndexV1::try_new(
        produced
            .protocols()
            .records()
            .iter()
            .flat_map(|r| r.parameters().parameters())
            .filter_map(|p| p.calling().template())
            .collect(),
    )
    .unwrap();
    let bytes = encode(
        &produced
            .protocols()
            .index_templates(&keys, &mut meter())
            .unwrap(),
    )
    .unwrap();
    let decoded: hir::DecodedCanonicalProtectedCallableSourceInterfacesV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let protocols = decoded
        .resolve(&mut fixture.identities, &keys, &mut meter())
        .unwrap();
    assert_eq!(
        encode(&protocols.index_templates(&keys, &mut meter()).unwrap()).unwrap(),
        bytes
    );
    (declarations, protocols)
}

pub(super) fn outline(
    foundation: &hir::BoundTypeFoundationSourcesV1<'_>,
    table: &hir::CanonicalProtectedDeclarationInterfacesV1,
) -> String {
    let mut rows = BTreeMap::<String, [usize; 4]>::new();
    for record in table.records() {
        let owner = *record.declaration_access().lexical_owners().last().unwrap();
        let scoop_identity::DeclarationName::Named(name) =
            foundation.nominal_key(owner).unwrap().name()
        else {
            panic!("nominal name");
        };
        let index = match record {
            Declaration::Callable(_) => 0,
            Declaration::Constructor(_) => 1,
            Declaration::Property(_) => 2,
            Declaration::NestedNominal(_) => 3,
        };
        rows.entry(name.as_str().to_owned()).or_default()[index] += 1;
    }
    rows.into_iter()
        .map(|(name, counts)| {
            format!(
                "{name}: callable={}, constructor={}, property={}, nested={}\n",
                counts[0], counts[1], counts[2], counts[3]
            )
        })
        .collect()
}

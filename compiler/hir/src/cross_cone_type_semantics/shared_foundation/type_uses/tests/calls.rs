use super::*;
use fixture::CallForm;

mod constructors;
mod negative;
mod receivers;
mod resources;
mod signatures;

#[test]
fn shared_call_signatures_derive_demand_for_core_and_ordinary_providers() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    for (coordinate, name) in [
        (ConeCoordinate::reserved_core(), "core"),
        (coordinate("provider"), "ordinary"),
    ] {
        let mut artifact = Artifact::new(coordinate);
        let owner = artifact.nominal("Owner", SourceNominalKind::Class, &[]);
        let unused = artifact.nominal("Unused", SourceNominalKind::Class, &[]);
        let mut provider = artifact.load(&[]);
        let member = provider.callable(nominal_owner(owner), "run", CallForm::Function);
        provider.callable(nominal_owner(unused), "unused", CallForm::Function);
        let dependencies = dependencies(&core, &provider);
        let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
        consumer.calls(&provider, &[member]);
        let actual = consumer.uses(&dependencies).unwrap();
        assert_eq!(
            actual,
            selected(signature_uses(provider.provider(), &[owner]))
        );
        consumer
            .validate(&actual, &dependencies, &mut meter())
            .unwrap();
        snapshot(&format!("call-signature-{name}"), &actual);
    }
}

#[test]
fn shared_call_signatures_preserve_unit_arguments_and_close_owner_ancestry_once() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("provider"));
    let base = source.nominal("Base", SourceNominalKind::Class, &[]);
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[base]);
    let value = source.nominal("Value", SourceNominalKind::Struct, &[]);
    let mut provider = source.load(&[&core]);
    let getter = provider.callable(nominal_owner(owner), "property", CallForm::Getter);
    let setter = provider.callable(nominal_owner(owner), "property", CallForm::Setter);
    let method = provider.callable(nominal_owner(value), "run", CallForm::Function);
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    consumer.calls(&provider, &[getter, method, setter, getter, method]);
    let actual = consumer.uses(&dependencies).unwrap();
    let mut expected = signature_uses(provider.provider(), &[owner, value]);
    expected.push(representation(provider.provider(), base));
    assert_eq!(actual, selected(expected));
    consumer
        .validate(&actual, &dependencies, &mut meter())
        .unwrap();
    let decoded: DecodedCanonicalSelectedExternalTypeUsesV1 = scoop_wire::decode_canonical(
        &scoop_wire::encode(&actual).unwrap(),
        DecodeLimits::default(),
    )
    .unwrap();
    assert_eq!(
        decoded
            .resolve(&mut consumer.identities, &mut meter(), &WirePath::root())
            .unwrap(),
        actual
    );
    snapshot("call-signature-combined", &actual);
}

#[test]
fn shared_call_signatures_follow_actual_top_level_and_extension_calls() {
    let core = Artifact::new(ConeCoordinate::reserved_core()).load(&[]);
    let mut source = Artifact::new(coordinate("provider"));
    let owner = source.nominal("Owner", SourceNominalKind::Class, &[]);
    let mut provider = source.load(&[&core]);
    provider.callable(nominal_owner(owner), "run", CallForm::Function);
    let top = provider.callable(
        PublicDeclarationOwnerV1::TopLevel,
        "top",
        CallForm::Function,
    );
    let extension = provider.callable(
        PublicDeclarationOwnerV1::Extension,
        "extension",
        CallForm::Function,
    );
    let dependencies = dependencies(&core, &provider);
    let mut consumer = Artifact::new(coordinate_for_consumer()).load(&dependencies);
    assert!(consumer.uses(&dependencies).unwrap().records().is_empty());
    assert!(matches!(
        consumer.validate(
            &selected(vec![signature(provider.provider(), owner)]),
            &dependencies,
            &mut meter()
        ),
        Err(Error::TypeUseInventory)
    ));
    consumer.calls(&provider, &[top, extension]);
    assert_eq!(
        consumer.uses(&dependencies).unwrap(),
        selected(signature_uses(provider.provider(), &[]))
    );
}

fn dependencies<'a>(core: &'a Loaded, provider: &'a Loaded) -> Vec<&'a Loaded> {
    if core.provider() == provider.provider() {
        vec![provider]
    } else {
        vec![core, provider]
    }
}

fn nominal_owner(owner: PersistentTypeId) -> PublicDeclarationOwnerV1 {
    PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner))
}

fn representation(provider: ConeIdentity, owner: PersistentTypeId) -> SelectedExternalTypeUseV1 {
    SelectedExternalTypeUseV1::new(
        provider,
        SelectedTypeUseV1::Representation {
            exact: exact(owner),
        },
    )
}

fn signature(provider: ConeIdentity, owner: PersistentTypeId) -> SelectedExternalTypeUseV1 {
    SelectedExternalTypeUseV1::new(
        provider,
        SelectedTypeUseV1::Signature {
            exact: exact(owner),
        },
    )
}

fn signature_uses(
    provider: ConeIdentity,
    owners: &[PersistentTypeId],
) -> Vec<SelectedExternalTypeUseV1> {
    let unit = CoreBuiltinNominal::Unit.identity_record().id();
    let mut uses = vec![
        representation(ConeIdentity::CORE, unit),
        signature(ConeIdentity::CORE, unit),
    ];
    for owner in owners {
        uses.push(representation(provider, *owner));
        uses.push(signature(provider, *owner));
    }
    uses
}

fn snapshot(name: &str, selected: &CanonicalSelectedExternalTypeUsesV1) {
    let text = selected
        .records()
        .iter()
        .map(|record| {
            let usage = match record.usage() {
                SelectedTypeUseV1::Representation { exact } => format!("Representation {exact}"),
                SelectedTypeUseV1::Signature { exact } => format!("Signature {exact}"),
                SelectedTypeUseV1::Construct { exact, declaration } => match declaration {
                    SelectedTypeConstructionV1::Constructor(id) => {
                        format!("Construct {exact} Constructor {id}")
                    }
                    SelectedTypeConstructionV1::EnumVariant(id) => {
                        format!("Construct {exact} EnumVariant {id}")
                    }
                },
                _ => panic!("the fixture selects signatures, representations and construction"),
            };
            format!("{} {usage}\n", record.provider())
        })
        .collect::<String>();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tests/fixtures/m23-shared-type-uses/{name}.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_USES").is_some() {
        std::fs::write(&path, &text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}

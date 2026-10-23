use std::rc::Rc;

use scoop_hir as hir;
use scoop_identity::{
    ConeCoordinate, ConeIdentity, PendingIdentityValidation, RequestedConeKind,
    SemanticIdentitySession,
};

use super::super::m23_ordinary_core_only::support::{parsed_ordinary_text_at, trusted_core};
use crate::{CurrentConeSources, lower_current_export, project_source_dependency};

#[test]
fn source_providers_use_ordinary_generic_default_alias_and_nominal_imports() {
    let core = trusted_core();
    let mut session = SemanticIdentitySession::new();
    let mut pending = PendingIdentityValidation::new();
    core.source_foundation
        .register_identities(&mut pending)
        .unwrap();
    let identities = session
        .import_source(ConeIdentity::CORE, &pending.finish().unwrap())
        .unwrap()
        .into_parts()
        .0;
    let core_foundation =
        hir::ImportedHirFoundation::from_shared(Rc::clone(&core.source_foundation), identities);
    let core_input = || hir::ImportedProviderInput {
        foundation: &core_foundation,
        interface: core.provider().interface,
        alias_expansions: core.provider().alias_expansions,
    };
    let coordinate = ConeCoordinate::new("test", "source-provider", "1.0.0").unwrap();
    let source = parsed_ordinary_text_at(
        &coordinate,
        r#"package shared
public struct Box<Element>(val value: Element)
public typealias Flag = Boolean
public const val ENABLED: Boolean = true
public fun <Item> pass(value: Item): Item = value
public fun choose(value: Flag = ENABLED): Boolean = pass(value)
@CLayout public struct Payload(val value: Boolean)
@Extern(name = "source_payload_copy") public fun nativeCopy(value: Payload): Payload
"#,
    );
    let world = hir::ImportedSemanticWorld::from_dependencies(
        source.cone(),
        vec![core_input()],
        Vec::new(),
    )
    .unwrap();
    let input = CurrentConeSources::try_new(
        &source,
        core_foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let output = lower_current_export(RequestedConeKind::Library, &input).unwrap();
    let provider = project_source_dependency(&output, &world, &mut session).unwrap();
    assert_eq!(session.origin_fingerprint(source.cone()), None);
    assert!(
        !provider
            .interface
            .generic_callable_bodies()
            .records()
            .is_empty()
    );
    assert!(!provider.interface.default_templates().records().is_empty());
    assert!(!provider.alias_expansions.is_empty());

    let coordinate = ConeCoordinate::new("test", "source-consumer", "1.0.0").unwrap();
    let consumer = parsed_ordinary_text_at(
        &coordinate,
        "import shared.Box\nimport shared.Flag\nimport shared.pass\nimport shared.choose\nimport shared.Payload\nimport shared.nativeCopy\npublic fun run(): Boolean { val box = Box(true); val flag: Flag = choose(); return pass(box.value) && flag }\npublic fun copy(value: Payload): Payload { @Unsafe { return nativeCopy(value) } }",
    );
    let world = hir::ImportedSemanticWorld::from_dependencies(
        consumer.cone(),
        vec![core_input(), provider.input()],
        Vec::new(),
    )
    .unwrap();
    let input = CurrentConeSources::try_new(
        &consumer,
        core_foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let consumed = lower_current_export(RequestedConeKind::Library, &input).unwrap();
    assert!(!consumed.export.imported_generic_applications.is_empty());
    assert!(
        consumed
            .imported_dependencies
            .callables()
            .any(|callable| callable.provider() == source.cone())
    );
    project_source_dependency(&consumed, &world, &mut session).unwrap();
}

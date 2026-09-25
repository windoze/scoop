use super::*;
use scoop_identity::{
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionAtomRole, DefinitionAtomSubkey,
    DefinitionOwnerChain, LinkageClass, ObjectDefinitionAtomKey, PackagePath,
    PersistentSymbolRequest, PersistentSymbolRequestTable, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{Encoder, WireEncode, encode};

const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

fn exact(name: &str) -> CborIdentityRecord<PersistentExactTypeId, ExactTypeKey> {
    let site = SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap();
    let source = SourceDeclarationKey::nominal(
        site,
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Struct,
        0,
    );
    let nominal = PersistentTypeId::from_source_declaration(&source).unwrap();
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap()
}

fn foundation(
    exact: PersistentExactTypeId,
    role: RepresentationRole,
    definition: bool,
) -> OdrFreeLirFoundation {
    let layout =
        CborIdentityRecord::from_key(LayoutKey::new(exact, TARGET.wire_id(), role)).unwrap();
    let (key, symbol) = ExternalStrongShapeSubjectV1::Layout(layout.id())
        .expected_definition(ConeIdentity::SINGLE_FILE)
        .unwrap();
    let mut canonical = crate::CanonicalLirFoundation::empty();
    canonical.set_layouts(vec![layout]).unwrap();
    if definition {
        let plan = CborIdentityRecord::from_key(key).unwrap();
        canonical
            .set_definition_atoms(vec![
                CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
                    plan.id(),
                    DefinitionAtomRole::Primary,
                    DefinitionAtomSubkey::Singleton,
                ))
                .unwrap(),
            ])
            .unwrap();
        canonical.set_definition_plans(vec![plan]).unwrap();
        canonical.set_symbol_requests(
            PersistentSymbolRequestTable::new(vec![
                PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap(),
            ])
            .unwrap(),
        );
    }
    OdrFreeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap()
}

#[test]
fn layout_binding_preserves_the_complete_physical_definition_and_legacy_wire() {
    let exact = exact("Value");
    for role in [
        RepresentationRole::ManagedValue,
        RepresentationRole::ManagedObject,
        RepresentationRole::CValue,
        RepresentationRole::NativeFunctionPointer,
    ] {
        let foundation = foundation(exact.id(), role, true);
        let bound =
            ExactLayoutIdentityV1::from_foundation(TARGET, exact.clone(), role, &foundation)
                .unwrap();
        assert_eq!(bound.exact(), exact.id());
        assert_eq!(bound.exact_key(), exact.key());
        assert_eq!(bound.layout_key().representation(), role);
        assert_eq!(bound.target(), TARGET);
        assert_eq!(
            bound.physical_definition().provider(),
            foundation.producer()
        );
        assert_eq!(
            bound.physical_definition().primary(),
            foundation.definition_atoms()[0].id()
        );
        struct LegacyDefinition(StrongShapeDefinitionV1<PersistentLayoutId>);
        impl WireEncode for LegacyDefinition {
            fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                encoder.map(3)?;
                encoder.field(1)?;
                self.0.semantic_id().encode(encoder)?;
                encoder.field(2)?;
                self.0.definition_plan().encode(encoder)?;
                encoder.field(3)?;
                self.0.symbol().encode(encoder)
            }
        }
        assert_eq!(
            encode(&bound.definition()).unwrap(),
            encode(&LegacyDefinition(bound.definition())).unwrap()
        );
        assert!(
            StrongShapeDefinitionV1::from_layout_definition(
                PersistentLayoutId::from_key(&LayoutKey::new(
                    self::exact("Other").id(),
                    TARGET.wire_id(),
                    role,
                ))
                .unwrap(),
                bound.physical_definition(),
            )
            .is_none()
        );
    }
}

#[test]
fn identity_binding_rejects_foreign_exact_role_and_missing_physical_definition() {
    let exact = exact("Value");
    let full = foundation(exact.id(), RepresentationRole::ManagedValue, true);
    for (record, role) in [
        (self::exact("Other"), RepresentationRole::ManagedValue),
        (exact.clone(), RepresentationRole::ManagedObject),
    ] {
        assert!(matches!(
            ExactLayoutIdentityV1::from_foundation(TARGET, record, role, &full),
            Err(ExactLayoutIdentityError::MissingLayout(_))
        ));
    }
    let absent = foundation(exact.id(), RepresentationRole::ManagedValue, false);
    assert!(matches!(
        ExactLayoutIdentityV1::from_foundation(
            TARGET,
            exact,
            RepresentationRole::ManagedValue,
            &absent,
        ),
        Err(ExactLayoutIdentityError::Definition(
            StrongShapeDefinitionError::MissingDefinition(_)
        ))
    ));
}

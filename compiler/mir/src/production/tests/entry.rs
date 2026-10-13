use super::*;

#[test]
fn generated_entry_bridge_keeps_the_selected_generated_implementation() {
    let mut fixture = fixture_at(ConeIdentity::SINGLE_FILE);
    let generated = CborIdentityRecord::from_key(GeneratedCallableKey::TestEntry {
        cone: ConeIdentity::SINGLE_FILE,
    })
    .unwrap();
    fixture
        .hir
        .set_generated_callables(vec![generated.clone()])
        .unwrap();
    let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![], fixture.exact_unit);
    let source = ExecutableEntryIdentity::for_test(&generated, signature.clone()).unwrap();
    let owner = CallableOwner::Generated(generated.id());
    let mut signatures = fixture.mir.callable_signatures().to_vec();
    signatures.push(CallableSignatureRecord::new(
        CallableSignatureSubject::Strong(owner),
        signature,
    ));
    fixture.mir.set_callable_signatures(signatures).unwrap();
    let section = CoreBootstrapBridgeSectionV1::try_new(
        ConeIdentity::SINGLE_FILE,
        EntryMirBridgeBranchV1::Executable(Box::new(
            EntryMirBridgeV1::new(source.clone(), owner).unwrap(),
        )),
        StrongCallableBridgeSurfaceV1::from_foundation(&fixture.mir),
    )
    .unwrap();
    let (mut identities, foundation) = validate_foundations(&fixture);
    assert_eq!(
        decode(&section).validate(ConeIdentity::SINGLE_FILE, &mut identities, &foundation),
        Ok(section)
    );
    assert!(matches!(
        EntryMirBridgeV1::new(source, CallableOwner::Function(fixture.function.id())),
        Err(MirProductionBuildError::EntryImplementationMismatch { .. })
    ));
}

#[test]
fn executable_entry_bridge_round_trips_the_complete_hir_proof() {
    let artifact = ConeIdentity::SINGLE_FILE;
    let function = CborIdentityRecord::from_key(source_function_in(artifact, "main")).unwrap();
    let unit_record = CoreBuiltinNominal::Unit.identity_record();
    let unit_type = unit_record.id();
    let exact_unit_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit_type)).unwrap();
    let source = entry_source(&function, exact_unit_record.id());
    let signature =
        ExactCallableSignature::new(Effect::Ordinary, None, Vec::new(), exact_unit_record.id());
    let mut hir = CanonicalHirFoundation::empty();
    hir.set_types(vec![unit_record]).unwrap();
    hir.set_functions(vec![function.clone()]).unwrap();
    hir.set_exact_types(vec![exact_unit_record]).unwrap();
    let mut mir = CanonicalMirFoundation::empty();
    mir.set_callable_signatures(vec![CallableSignatureRecord::new(
        CallableSignatureSubject::Strong(CallableOwner::Function(function.id())),
        signature,
    )])
    .unwrap();
    let bridges = StrongCallableBridgeSurfaceV1::from_foundation(&mir);
    let section = CoreBootstrapBridgeSectionV1::try_new(
        artifact,
        EntryMirBridgeBranchV1::Executable(Box::new(
            EntryMirBridgeV1::new(source.clone(), CallableOwner::Function(function.id())).unwrap(),
        )),
        bridges,
    )
    .unwrap();
    let bytes = encode(&section).unwrap();
    assert_eq!(
        hex(&bytes),
        "a202a2000201a201a501582000769af7cd4a85d98841cd63dabb8e73c4d41bce56f5e225f7295dd27c3f39b602a200010158209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd03a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0458205a43bee43f27e5c33d012c1129702324d18dd3856d3158b657383cc2d61c9257055820a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca02a200010158209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd0381a301a200010158209714f93e5ddfe8681524ef2048f90a96e0e87dcbc813b55879c836164da02cdd02a4010102a1000103800458201dff58a7007c61d14decc85852d44e40d113b26e96ec4d24b365bcde341966dc0301"
    );

    let (mut identities, foundation) = validate_mir(&hir, &mir);
    let validated = decode(&section)
        .validate(artifact, &mut identities, &foundation)
        .unwrap();
    let EntryMirBridgeBranchV1::Executable(entry) = validated.entry_bridge() else {
        panic!("the executable bridge remains executable")
    };
    assert_eq!(entry.source(), &source);

    let mut tampered = bytes;
    let body = source.main().body();
    let offset = tampered
        .windows(body.as_array().len())
        .position(|window| window == body.as_array())
        .unwrap();
    tampered[offset + body.as_array().len() - 1] ^= 1;
    let decoded: DecodedCoreBootstrapBridgeSectionV1 = decode_canonical(&tampered).unwrap();
    let (mut identities, foundation) = validate_mir(&hir, &mir);
    assert_eq!(
        decoded.validate(artifact, &mut identities, &foundation),
        Err(MirProductionValidationError::EntrySourceMismatch)
    );
}

#[test]
fn entry_bridge_rejects_the_removed_id_only_wire() {
    let function =
        CborIdentityRecord::from_key(source_function_in(ConeIdentity::SINGLE_FILE, "main"))
            .unwrap();
    let bytes = encode(&IdOnlyEntryBranch(function.id())).unwrap();
    assert!(decode_canonical::<DecodedEntryMirBridgeBranchV1>(&bytes).is_err());
}

struct IdOnlyEntryBranch(PersistentFunctionId);

impl scoop_wire::WireEncode for IdOnlyEntryBranch {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(2)?;
        encoder.field(1)?;
        encoder.map(2)?;
        encoder.field(1)?;
        self.0.encode(encoder)?;
        encoder.field(2)?;
        CallableOwner::Function(self.0).encode(encoder)
    }
}

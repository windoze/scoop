use super::*;

#[test]
fn program_link_reads_machine_data_from_actual_executable_artifacts() {
    let target = resolved_target().expect("program Link fixtures require the target toolchain");
    let workspace = tempfile::tempdir().unwrap();
    let core = bootstrap_core(workspace.path(), &target);
    let core_bytes = std::fs::read(core.artifact().path()).unwrap();
    for case in ["read-basic", "read-combined"] {
        let source = std::fs::read_to_string(
            crate::workspace_root().join(format!("tests/fixtures/m23-program-link/{case}.scoop")),
        )
        .unwrap();
        let root = workspace.path().join(case);
        write_manifest_cone(&root, "dev.example", case, "executable", &source);
        let executable = build_manifest(
            workspace.path(),
            &target,
            &root,
            &workspace.path().join(format!("output/{case}.slib")),
        );
        let bytes = std::fs::read(executable.artifact().path()).unwrap();
        let closure = scoop_slib::read_program_link_closure(
            &bytes,
            &[&core_bytes, &core_bytes],
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile(),
        )
        .unwrap_or_else(|error| panic!("{case}: {error}"));
        assert_eq!(closure.artifacts().len(), 2);
        let (core, core_symbols) = closure.artifacts().next().unwrap();
        assert_eq!(core_symbols.link_support().runtime_data_aliases().len(), 1);
        let alias_bytes = scoop_wire::encode(core_symbols.link_support()).unwrap();
        let decoded = scoop_wire::decode_canonical::<scoop_slib::DecodedLirLinkSupportSectionV1>(
            &alias_bytes,
        )
        .unwrap();
        assert_eq!(
            &decoded
                .read_link(core.layout().exports().descriptors())
                .unwrap(),
            core_symbols.link_support()
        );
        check_invalid_aliases(core, core_symbols.link_support());
        let (root, symbols) = closure.artifacts().last().unwrap();
        assert_eq!(root.identity(), closure.root());
        assert!(matches!(
            root.production().entry_plan(),
            scoop_lir::EntryProductionPlanV1::Executable(_)
        ));
        assert_eq!(
            scoop_slib::FingerprintAvailability::Available(symbols.code_fingerprint()),
            root.manifest().semantic_fingerprints().code()
        );
        let missing = scoop_slib::read_program_link_closure(
            &bytes,
            &[],
            target.lir_target_selection(),
            target.c_bridge_toolchain().profile(),
        )
        .err()
        .unwrap();
        assert!(missing.to_string().contains("MissingDirectArtifact"));
    }
    let library_root = scoop_slib::read_program_link_closure(
        &core_bytes,
        &[],
        target.lir_target_selection(),
        target.c_bridge_toolchain().profile(),
    )
    .err()
    .unwrap();
    assert!(library_root.to_string().contains("executable Cone"));
}

fn check_invalid_aliases(
    core: &scoop_slib::ProgramLinkArtifact,
    support: &scoop_slib::LirLinkSupportSectionV1,
) {
    use scoop_identity::StrongDefinitionRole;
    let alias = &support.runtime_data_aliases()[0];
    let descriptors = core.layout().exports().descriptors();
    let non_string = descriptors
        .records()
        .iter()
        .find(|record| {
            !matches!(
                record.instance_layout().representation().kind(),
                scoop_lir::InstanceRepresentationKindV1::InlineBytes
            )
        })
        .unwrap();
    let valid = AliasFixture {
        contract: *alias.contract().as_array(),
        entity: alias.owner().entity(),
        role: alias.owner().role(),
        count: 1,
    };
    for (fixture, expected) in [
        (AliasFixture { count: 2, ..valid }, "duplicate"),
        (
            AliasFixture {
                contract: [0; 32],
                ..valid
            },
            "unknown runtime data alias contract",
        ),
        (
            AliasFixture {
                role: StrongDefinitionRole::CallableBody,
                ..valid
            },
            "not a TypeDescriptor owner",
        ),
        (
            AliasFixture {
                entity: scoop_identity::StrongDefinitionEntity::exact_type(non_string.exact()),
                ..valid
            },
            "InlineBytes",
        ),
    ] {
        let bytes = scoop_wire::encode(&fixture).unwrap();
        let error =
            scoop_wire::decode_canonical::<scoop_slib::DecodedLirLinkSupportSectionV1>(&bytes)
                .unwrap()
                .read_link(descriptors)
                .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[derive(Clone, Copy)]
struct AliasFixture {
    contract: [u8; 32],
    entity: scoop_identity::StrongDefinitionEntity,
    role: scoop_identity::StrongDefinitionRole,
    count: u64,
}

impl scoop_wire::WireEncode for AliasFixture {
    fn encode(&self, e: &mut scoop_wire::Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(1)?;
        e.field(1)?;
        e.array(self.count)?;
        for _ in 0..self.count {
            e.map(2)?;
            e.field(1)?;
            e.bytes(&self.contract)?;
            e.field(2)?;
            e.map(2)?;
            e.field(1)?;
            self.entity.encode(e)?;
            e.field(2)?;
            self.role.encode(e)?;
        }
        Ok(())
    }
}

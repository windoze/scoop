use super::*;

#[test]
fn atomic_instances_keep_hidden_storage_and_reference_scans_through_wire() {
    for kind in AtomicValueKind::ALL.iter().copied() {
        let owner = source(
            kind.source_name(),
            SourceNominalKind::Class,
            u32::from(kind == AtomicValueKind::Reference),
        );
        let identity = if kind == AtomicValueKind::Reference {
            CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
                origin: PersistentGenericTypeId::from_source_declaration(&owner).unwrap(),
                arguments: NonEmptyVec::from_first(managed().identity().exact(), []),
            })
            .unwrap()
        } else {
            exact(&owner)
        };
        let bound = Bound::instance(identity);
        let layout =
            ExactInstanceLayoutV1::atomic(bound.identity, kind, &bound.foundation).unwrap();
        assert_eq!(
            layout.shape().instance_kind(),
            TypeInstanceKindV1::FixedObject
        );
        assert!(
            matches!(layout.representation().kind(), InstanceRepresentationKindV1::Atomic(actual) if actual == kind)
        );
        assert_wire_roundtrip(layout);
    }
}

#[test]
fn atomic_storage_matches_the_supported_target_abi() {
    for target in [
        LirTargetProfile::DARWIN_AARCH64,
        LirTargetProfile::LINUX_X86_64_GNU,
        LirTargetProfile::LINUX_X86_64_MUSL,
    ] {
        for kind in AtomicValueKind::ALL.iter().copied() {
            let layout = atomic_object_layout(target, kind).unwrap();
            assert_eq!(
                (layout.size, layout.align, layout.value.offset),
                (24, 8, 16)
            );
            assert_eq!(layout.value.access_align, u64::from(kind.bytes()));
            assert_eq!(
                layout.scan,
                if kind == AtomicValueKind::Reference {
                    RefScan::References(vec![16])
                } else {
                    RefScan::None
                }
            );
        }
    }
}

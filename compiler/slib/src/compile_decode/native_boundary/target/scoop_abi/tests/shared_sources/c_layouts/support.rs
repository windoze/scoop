use super::*;
use scoop_hir::{HirCLayoutContract, HirCLayoutValue, NominalSourceFieldV1};
use scoop_identity::SignatureTypeKey;
use scoop_lir::CanonicalLirFoundation;

pub(super) struct DeclaredLayout {
    pub current: Fixture,
    pub scalar: Fixture,
    cone: crate::ConeRecord,
    exact: PersistentExactTypeId,
    field: PersistentFieldId,
}

impl DeclaredLayout {
    pub fn new() -> Self {
        let cone = crate::strong_compile_decode::tests::cone_named("shared-c-layout");
        let mut current = Fixture::nominal(cone.identity(), false).without_native_witness();
        let scalar = Fixture::intrinsic(
            ConeIdentity::CORE,
            scoop_hir::IntrinsicTypeKind::Integer(scoop_hir::IntegerKind::SIGNED_64),
        )
        .without_native_witness();
        let exact = current.exact();
        let owner = nominal(&current);
        let source = current
            .identities
            .canonical_key::<_, SourceDeclarationKey>(owner)
            .unwrap();
        let field = CborIdentityRecord::from_key(
            FieldIdentityKey::source_declared(&source, CanonicalIdentifier::new("value").unwrap())
                .unwrap(),
        )
        .unwrap();
        let field_id = field.id();
        let shape = NominalSourceShapeV1::Struct(
            StructSourceShapeV1::try_new(
                vec![NominalSourceFieldV1::new(
                    field_id,
                    SignatureTypeKey::Nominal(nominal(&scalar)),
                )],
                NominalCLayoutPolicyV1::CLayout {
                    contract: HirCLayoutContract {
                        aligned: HirCLayoutValue::A16,
                        packed: HirCLayoutValue::A2,
                    },
                },
                true,
            )
            .unwrap(),
        );
        current.nominals = CanonicalNominalInterfacesV1::try_new(vec![
            crate::nominal_interface_fixture::public_record(
                SourceNominalId::Concrete(owner),
                shape.kind(),
                CanonicalBinderListV1::try_new(vec![]).unwrap(),
                CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
                CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
                CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
                CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
                shape,
            )
            .unwrap(),
        ])
        .unwrap();
        let mut foundation = current.foundation.as_canonical().clone();
        foundation.set_fields(vec![field]).unwrap();
        current.foundation = OdrFreeHirFoundation::try_new(foundation).unwrap();
        let decoded = [&scalar.foundation, &current.foundation].map(|foundation| {
            decode_canonical::<DecodedHirFoundation>(&encode(foundation).unwrap()).unwrap()
        });
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(scalar.identity).unwrap();
        pending.register_authority(current.identity).unwrap();
        for foundation in &decoded {
            foundation.register_identities(&mut pending).unwrap();
        }
        for foundation in &decoded {
            foundation.resolve_identities(&mut pending).unwrap();
        }
        current.identities = pending.finish().unwrap();
        Self {
            current,
            scalar,
            cone,
            exact,
            field: field_id,
        }
    }

    pub fn contract(&self, offset: u64) -> CanonicalCAbiLayoutFingerprintRecord {
        CanonicalCAbiLayoutFingerprintRecord::new(CanonicalCAbiLayout::new(
            self.exact,
            16,
            std::num::NonZeroU64::new(16).unwrap(),
            CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes16),
            CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes2),
            vec![CanonicalCAbiLayoutField::new(
                self.field,
                offset,
                CanonicalCStorageType::Integer {
                    exact_type: self.scalar.exact(),
                    signedness: Signedness::Signed,
                    bit_width: IntegerBitWidth::Bits64,
                },
            )],
        ))
        .unwrap()
    }

    pub fn check(
        &self,
        layouts: &[CanonicalCAbiLayoutFingerprintRecord],
        materialized: bool,
        dependency: bool,
    ) -> Result<(), NativeBoundaryCompileError> {
        let target = scoop_lir::ValidatedLirTargetSelection::DARWIN_AARCH64_LLVM_22_1;
        let artifact =
            crate::IdentityFoundationArtifact::write(crate::IdentityFoundationArtifactInput::new(
                crate::ProducerRecord::new("test").unwrap(),
                self.cone.clone(),
                target,
                self.current.foundation.as_canonical(),
                &scoop_mir::CanonicalMirFoundation::empty(),
                &CanonicalLirFoundation::empty(),
            ))
            .unwrap();
        let mut graph = crate::strong_compile_decode::tests::open_graph(artifact.as_bytes());
        let view = crate::NativeBoundaryFoundationView {
            source_contracts: &[],
            type_definitions: &[],
            callback_applications: &[],
            native_contracts: &[],
            c_abi_signatures: &[],
            c_abi_layouts: layouts,
            callback_bridges: &[],
        };
        let dependencies = dependency
            .then(|| self.scalar.borrow())
            .into_iter()
            .collect::<Vec<_>>();
        let materialized = materialized
            .then_some(self.exact)
            .into_iter()
            .collect::<Vec<_>>();
        crate::compile_decode::validate_shared_native_boundary_parts(
            &mut graph,
            self.current.borrow(),
            &dependencies,
            &view,
            &materialized,
        )?;
        Ok(())
    }
}

fn nominal(fixture: &Fixture) -> PersistentTypeId {
    let key = fixture
        .identities
        .canonical_key::<_, ExactTypeKey>(fixture.exact())
        .unwrap();
    let ExactTypeKey::Nominal(owner) = *key else {
        panic!("the fixture declares a nominal")
    };
    owner
}

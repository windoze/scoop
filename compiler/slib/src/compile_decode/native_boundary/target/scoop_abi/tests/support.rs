use super::*;

pub(super) struct Fixture {
    pub identity: ConeIdentity,
    pub nominals: scoop_hir::CanonicalNominalInterfacesV1,
    pub identities: ValidatedIdentityGraph,
    pub foundation: OdrFreeHirFoundation,
}

impl Fixture {
    pub fn empty() -> Self {
        Self {
            identity: ConeIdentity::SINGLE_FILE,
            nominals: scoop_hir::CanonicalNominalInterfacesV1::default(),
            identities: PendingIdentityValidation::new().finish().unwrap(),
            foundation: OdrFreeHirFoundation::try_new(CanonicalHirFoundation::empty()).unwrap(),
        }
    }

    pub fn nominal(provider: ConeIdentity, reference: bool) -> Self {
        let (kind, shape) = if reference {
            (
                SourceNominalKind::Class,
                NativeBoundaryNominalShape::Reference,
            )
        } else {
            (
                SourceNominalKind::Struct,
                NativeBoundaryNominalShape::Struct {
                    c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                    fields: vec![],
                },
            )
        };
        Self::from_shape(provider, kind, shape)
    }

    pub fn intrinsic(provider: ConeIdentity, family: scoop_hir::IntrinsicTypeKind) -> Self {
        let kind = match family.target() {
            scoop_hir::IntrinsicTypeTarget::Class => SourceNominalKind::Class,
            scoop_hir::IntrinsicTypeTarget::Struct => SourceNominalKind::Struct,
        };
        Self::from_shape(
            provider,
            kind,
            NativeBoundaryNominalShape::Intrinsic(
                scoop_hir::NominalIntrinsicRepresentationV1::new(family),
            ),
        )
    }

    fn from_shape(
        provider: ConeIdentity,
        kind: SourceNominalKind,
        shape: NativeBoundaryNominalShape,
    ) -> Self {
        let source = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("SameName").unwrap(),
            kind,
            0,
        );
        let nominals = match &shape {
            NativeBoundaryNominalShape::Intrinsic(representation) => {
                use scoop_hir::*;
                let source_shape = NominalSourceShapeV1::Intrinsic(*representation);
                CanonicalNominalInterfacesV1::try_new(vec![
                    crate::nominal_interface_fixture::public_record(
                        SourceNominalId::Concrete(
                            PersistentTypeId::from_source_declaration(&source).unwrap(),
                        ),
                        source_shape.kind(),
                        CanonicalBinderListV1::try_new(vec![]).unwrap(),
                        CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
                        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
                        CanonicalPublicMemberRefsV1::try_new(vec![]).unwrap(),
                        CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
                        source_shape,
                    )
                    .unwrap(),
                ])
                .unwrap()
            }
            _ => scoop_hir::CanonicalNominalInterfacesV1::default(),
        };
        let mut foundation = CanonicalHirFoundation::empty();
        foundation
            .set_types(vec![CborIdentityRecord::from_key(source.clone()).unwrap()])
            .unwrap();
        foundation
            .set_exact_types(vec![
                CborIdentityRecord::from_key(ExactTypeKey::Nominal(
                    PersistentTypeId::from_source_declaration(&source).unwrap(),
                ))
                .unwrap(),
            ])
            .unwrap();
        foundation
            .set_native_boundary_types(vec![
                NativeBoundaryTypeDefinitionRecord::new(&source, &[0], shape).unwrap(),
            ])
            .unwrap();
        let decoded: DecodedHirFoundation =
            decode_canonical(&encode(&foundation).unwrap(), DecodeLimits::default()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending.register_authority(provider).unwrap();
        decoded.register_identities(&mut pending).unwrap();
        decoded.resolve_identities(&mut pending).unwrap();
        Self {
            identity: provider,
            nominals,
            identities: pending.finish().unwrap(),
            foundation: OdrFreeHirFoundation::try_new(foundation).unwrap(),
        }
    }

    pub fn exact(&self) -> PersistentExactTypeId {
        let records = self
            .identities
            .records::<PersistentExactTypeId, ExactTypeKey>(
                IdentityLayer::Hir,
                &mut meter(),
                &WirePath::root(),
            )
            .unwrap();
        assert_eq!(records.len(), 1);
        records[0].id()
    }

    pub fn without_native_witness(mut self) -> Self {
        let mut foundation = self.foundation.as_canonical().clone();
        foundation.set_native_boundary_types(vec![]).unwrap();
        self.foundation = OdrFreeHirFoundation::try_new(foundation).unwrap();
        self
    }

    pub fn borrow(&self) -> AbiReplayDependency<'_> {
        AbiReplayDependency {
            identity: self.identity,
            identities: &self.identities,
            foundation: &self.foundation,
            nominals: &self.nominals,
        }
    }

    pub fn with_c_layout(&self) -> Self {
        let mut fixture = Self::nominal(ConeIdentity::SINGLE_FILE, false);
        assert_eq!(self.exact(), fixture.exact());
        let mut foundation = fixture.foundation.as_canonical().clone();
        let scoop_hir::NativeBoundaryNominalOwner::Concrete(owner) =
            fixture.foundation.native_boundary_types()[0].owner()
        else {
            panic!("concrete fixture")
        };
        let source = fixture
            .identities
            .canonical_key::<_, SourceDeclarationKey>(owner)
            .unwrap();
        let definition = NativeBoundaryTypeDefinitionRecord::new(
            &source,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::CLayout {
                    aligned: scoop_identity::CLayoutOverride::Natural,
                    packed: scoop_identity::CLayoutOverride::Natural,
                },
                fields: vec![],
            },
        )
        .unwrap();
        foundation
            .set_native_boundary_types(vec![definition])
            .unwrap();
        fixture.foundation = OdrFreeHirFoundation::try_new(foundation).unwrap();
        fixture
    }
}

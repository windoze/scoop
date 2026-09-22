use super::*;

pub(super) struct Fixture {
    pub identities: ValidatedIdentityGraph,
    pub foundation: OdrFreeHirFoundation,
}

impl Fixture {
    pub fn empty() -> Self {
        Self {
            identities: PendingIdentityValidation::new().finish().unwrap(),
            foundation: OdrFreeHirFoundation::try_new(CanonicalHirFoundation::empty()).unwrap(),
        }
    }

    pub fn nominal(provider: ConeIdentity, reference: bool) -> Self {
        let source = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("SameName").unwrap(),
            if reference {
                SourceNominalKind::Class
            } else {
                SourceNominalKind::Struct
            },
            0,
        );
        let shape = if reference {
            NativeBoundaryNominalShape::Reference
        } else {
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: vec![],
            }
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
            identities: pending.finish().unwrap(),
            foundation: OdrFreeHirFoundation::try_new(foundation).unwrap(),
        }
    }

    pub fn exact(&self) -> PersistentExactTypeId {
        let scoop_hir::NativeBoundaryNominalOwner::Concrete(owner) =
            self.foundation.native_boundary_types()[0].owner()
        else {
            panic!("concrete fixture")
        };
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(owner)).unwrap()
    }

    pub fn borrow(&self) -> AbiReplayDependency<'_> {
        AbiReplayDependency {
            identities: &self.identities,
            foundation: &self.foundation,
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

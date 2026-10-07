use super::*;
use scoop_identity::PersistentGenericTypeId;

mod c_projection;
mod nullable_projection;

struct Fixture {
    exact: PersistentExactTypeId,
    scalar: PersistentExactTypeId,
    unit: PersistentExactTypeId,
    records: Vec<NativeBoundaryTypeDefinitionRecord>,
    exact_types: HashMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
}

impl scoop_identity::PersistentIdResolver<PersistentExactTypeId> for Fixture {
    type Error = ();

    fn resolve(
        &mut self,
        id: scoop_identity::DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.exact_types
            .keys()
            .find_map(|exact| id.verify(*exact).ok())
            .ok_or(())
    }
}

impl Fixture {
    fn new(provider: ConeIdentity, name: &str, field_kind: scoop_hir::IntegerKind) -> Self {
        let declaration = |provider, name, arity| {
            SourceDeclarationKey::nominal(
                SourceDeclarationSite::new(
                    provider,
                    PackagePath::root(),
                    DefinitionOwnerChain::top_level(),
                    DeclarationScope::ConeWide,
                )
                .unwrap(),
                CanonicalIdentifier::new(name).unwrap(),
                SourceNominalKind::Struct,
                arity,
            )
        };
        let source = declaration(provider, name, 1);
        let scalar_source = declaration(ConeIdentity::SINGLE_FILE, "RawBits", 0);
        let scalar_owner = PersistentTypeId::from_source_declaration(&scalar_source).unwrap();
        let scalar_record =
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(scalar_owner)).unwrap();
        let unit_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Unit
                .identity_record()
                .id(),
        ))
        .unwrap();
        let argument_record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(
            scoop_identity::CoreBuiltinNominal::Any
                .identity_record()
                .id(),
        ))
        .unwrap();
        let application = CborIdentityRecord::from_key(ExactTypeKey::NominalApplication {
            origin: PersistentGenericTypeId::from_source_declaration(&source).unwrap(),
            arguments: NonEmptyVec::from_first(argument_record.id(), []),
        })
        .unwrap();
        let field =
            FieldIdentityKey::source_declared(&source, CanonicalIdentifier::new("raw").unwrap())
                .unwrap();
        let wrapper = NativeBoundaryTypeDefinitionRecord::new(
            &source,
            &[1],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: vec![
                    NativeBoundaryFieldDefinition::new(
                        &field,
                        SignatureTypeKey::Nominal(scalar_owner),
                    )
                    .unwrap(),
                ],
            },
        )
        .unwrap();
        let scalar = NativeBoundaryTypeDefinitionRecord::new(
            &scalar_source,
            &[0],
            NativeBoundaryNominalShape::Intrinsic(
                scoop_hir::NominalIntrinsicRepresentationV1::new(
                    scoop_hir::IntrinsicTypeKind::Integer(field_kind),
                ),
            ),
        )
        .unwrap();
        Self {
            exact: application.id(),
            scalar: scalar_record.id(),
            unit: unit_record.id(),
            records: vec![wrapper, scalar, super::builtins::any_definition()],
            exact_types: [application, scalar_record, argument_record, unit_record]
                .into_iter()
                .map(|record| (record.id(), record.into_shared_key()))
                .collect(),
        }
    }

    fn with_normalizer<T>(&self, run: impl FnOnce(&mut NativeBoundaryNormalizer<'_>) -> T) -> T {
        let definitions = self
            .records
            .iter()
            .map(|r| (r.owner(), AbiNominalDefinition::native(r)))
            .collect();
        let callable_applications = HashMap::new();
        let initialization_units = HashMap::new();

        let mut normalizer = NativeBoundaryNormalizer::new(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            &self.exact_types,
            &callable_applications,
            &initialization_units,
            &definitions,
        );
        run(&mut normalizer)
    }
}

#[test]
fn handle_named_structs_use_declared_scoop_aggregate_layout_from_either_provider() {
    for provider in [ConeIdentity::CORE, ConeIdentity::SINGLE_FILE] {
        for name in ["PinnedPtr", "GcHandle"] {
            for kind in [
                scoop_hir::IntegerKind::UNSIGNED_64,
                scoop_hir::IntegerKind::UNSIGNED_8,
            ] {
                let fixture = Fixture::new(provider, name, kind);
                fixture.with_normalizer(|normalizer| {
                    let size = u64::from(kind.width().bytes());
                    let layout = normalizer.scoop_layout(fixture.exact).unwrap();
                    assert_eq!(layout.size, size);
                    assert_eq!(layout.alignment, size);
                    assert_eq!(layout.shape, ScoopAbiValueShape::Aggregate);
                    assert!(layout.gc_free);
                    assert!(matches!(
                        normalizer.scoop_argument(fixture.exact).unwrap(),
                        ScoopAbiArgument::Indirect(storage)
                            if storage.exact_type() == fixture.exact && storage.byte_size() == size
                    ));
                    assert!(matches!(
                        normalizer.scoop_return(fixture.exact).unwrap(),
                        ScoopAbiReturn::Indirect(storage)
                            if storage.exact_type() == fixture.exact && storage.byte_size() == size
                    ));
                });
            }
        }
    }
}

#[test]
fn handle_identity_does_not_bypass_missing_nominal_or_field_witnesses() {
    for name in ["PinnedPtr", "GcHandle"] {
        for missing in [0, 1] {
            let mut fixture = Fixture::new(
                ConeIdentity::CORE,
                name,
                scoop_hir::IntegerKind::UNSIGNED_64,
            );
            let owner = fixture.records.remove(missing).owner();
            fixture.with_normalizer(|normalizer| {
                assert!(matches!(
                    normalizer.scoop_argument(fixture.exact),
                    Err(NativeBoundaryCompileError::ClosureRequired { owner: actual })
                        if actual == owner
                ));
            });
        }
    }
}

#[test]
fn mixed_handle_scalar_and_unit_parameters_preserve_the_complete_scoop_abi() {
    let mut dump = String::new();
    for name in ["PinnedPtr", "GcHandle"] {
        let fixture = Fixture::new(
            ConeIdentity::CORE,
            name,
            scoop_hir::IntegerKind::UNSIGNED_64,
        );
        fixture.with_normalizer(|normalizer| {
            let arguments = [fixture.scalar, fixture.exact, fixture.unit, fixture.exact]
                .into_iter()
                .map(|exact| normalizer.scoop_argument(exact).unwrap())
                .collect::<Vec<_>>();
            let signature = CanonicalScoopAbiFunctionSignature::new(
                ExactCallableSignature::new(
                    scoop_identity::Effect::Ordinary,
                    None,
                    vec![fixture.scalar, fixture.exact, fixture.unit, fixture.exact],
                    fixture.exact,
                ),
                arguments,
                normalizer.scoop_return(fixture.exact).unwrap(),
                GcEffect::Managed,
            )
            .unwrap();
            assert!(matches!(
                signature.arguments(),
                [
                    ScoopAbiArgument::Direct(_),
                    ScoopAbiArgument::Indirect(_),
                    ScoopAbiArgument::ElidedZst(_),
                    ScoopAbiArgument::Indirect(_)
                ]
            ));
            assert!(matches!(signature.result(), ScoopAbiReturn::Indirect(_)));
            let arguments = signature
                .arguments()
                .iter()
                .map(|argument| match argument {
                    ScoopAbiArgument::Direct(value) => storage("direct", *value),
                    ScoopAbiArgument::Indirect(value) => storage("indirect", *value),
                    ScoopAbiArgument::ElidedZst(value) => storage("elided", *value),
                })
                .collect::<Vec<_>>()
                .join(",");
            let result = match signature.result() {
                ScoopAbiReturn::UnitVoid => "unit".to_owned(),
                ScoopAbiReturn::Direct(value) => storage("direct", value),
                ScoopAbiReturn::Indirect(value) => storage("indirect", value),
                ScoopAbiReturn::ElidedZst(value) => storage("elided", value),
            };
            dump.push_str(&format!(
                "{name}: {:?} args=[{arguments}] result={result}\n",
                signature.gc_effect()
            ));
            let bytes = scoop_wire::encode(&signature).unwrap();
            let decoded: scoop_identity::DecodedCanonicalScoopAbiFunctionSignature =
                scoop_wire::decode_canonical(&bytes).unwrap();
            let mut resolver = Fixture::new(
                ConeIdentity::CORE,
                name,
                scoop_hir::IntegerKind::UNSIGNED_64,
            );
            assert_eq!(decoded.resolve(&mut resolver).unwrap(), signature);
        });
    }
    assert_eq!(
        dump,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-native-boundary/handle-scoop-abi.snap"
        ))
    );
}

fn storage(passing: &str, value: CanonicalScoopStorage) -> String {
    format!("{passing}:{}:{:?}", value.byte_size(), value.shape())
}

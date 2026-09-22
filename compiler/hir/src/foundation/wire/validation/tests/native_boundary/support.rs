use super::*;

pub(super) struct Fixture {
    provider: CanonicalHirFoundation,
    declaration: SourceDeclarationKey,
    record: NativeBoundaryTypeDefinitionRecord,
}

impl Fixture {
    pub(super) fn new(provider: ConeIdentity, generic: bool, enumeration: bool) -> Self {
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                provider,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("Payload").unwrap(),
            if enumeration {
                SourceNominalKind::Enum
            } else {
                SourceNominalKind::Struct
            },
            u32::from(generic),
        );
        let mut canonical = CanonicalHirFoundation::empty();
        if generic {
            canonical
                .set_generic_types(vec![
                    CborIdentityRecord::from_key(declaration.clone()).unwrap(),
                ])
                .unwrap();
        } else {
            canonical
                .set_types(vec![
                    CborIdentityRecord::from_key(declaration.clone()).unwrap(),
                ])
                .unwrap();
        }
        let value = SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id());
        let shape = if enumeration {
            let variant = CborIdentityRecord::from_key(
                EnumVariantIdentityKey::source(
                    &declaration,
                    CanonicalIdentifier::new("Some").unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let field = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
                variant.id(),
                EnumVariantFieldSelector::Positional {
                    declaration_index: 0,
                },
            ))
            .unwrap();
            let shape = NativeBoundaryNominalShape::Enum {
                variants: vec![
                    NativeBoundaryVariantDefinition::new(
                        variant.key(),
                        vec![
                            NativeBoundaryVariantFieldDefinition::new(field.key(), value).unwrap(),
                        ],
                    )
                    .unwrap(),
                ],
            };
            canonical.set_enum_variants(vec![variant]).unwrap();
            canonical.set_enum_variant_fields(vec![field]).unwrap();
            shape
        } else {
            let field = CborIdentityRecord::from_key(
                FieldIdentityKey::source_declared(
                    &declaration,
                    CanonicalIdentifier::new("value").unwrap(),
                )
                .unwrap(),
            )
            .unwrap();
            let shape = NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::CLayout {
                    aligned: CLayoutOverride::Natural,
                    packed: CLayoutOverride::Natural,
                },
                fields: vec![NativeBoundaryFieldDefinition::new(field.key(), value).unwrap()],
            };
            canonical.set_fields(vec![field]).unwrap();
            shape
        };
        let record =
            NativeBoundaryTypeDefinitionRecord::new(&declaration, &[u32::from(generic)], shape)
                .unwrap();
        Self {
            provider: canonical,
            declaration,
            record,
        }
    }

    pub(super) fn input(&self) -> CanonicalHirFoundation {
        let mut input = CanonicalHirFoundation::empty();
        input
            .set_types(vec![
                CoreBuiltinNominal::Unit.identity_record(),
                CoreBuiltinNominal::Any.identity_record(),
            ])
            .unwrap();
        let unit = ExactTypeRecord::from_key(ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .unwrap();
        let exact = match self.record.owner() {
            crate::NativeBoundaryNominalOwner::Concrete(id) => {
                input.set_external_source_types(vec![id]).unwrap();
                ExactTypeKey::Nominal(id)
            }
            crate::NativeBoundaryNominalOwner::GenericTemplate(origin) => {
                input.set_external_generic_types(vec![origin]).unwrap();
                ExactTypeKey::NominalApplication {
                    origin,
                    arguments: NonEmptyVec::new(vec![unit.id()]).unwrap(),
                }
            }
        };
        input
            .set_exact_types(vec![unit, ExactTypeRecord::from_key(exact).unwrap()])
            .unwrap();
        input
            .set_native_boundary_types(vec![self.record.clone()])
            .unwrap();
        input
    }

    pub(super) fn graph(
        &self,
        input: &DecodedHirFoundation,
        current: &ConeCoordinate,
        include_provider: bool,
    ) -> ValidatedIdentityGraph {
        let provider = decode(&self.provider);
        let provider = validate_identities(&provider, [self.declaration.origin()]);
        let mut pending = PendingIdentityValidation::new();
        for authority in [
            ConeIdentity::CORE,
            current.identity().unwrap(),
            self.declaration.origin(),
        ]
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>()
        {
            pending.register_authority(authority).unwrap();
        }
        input.register_identities(&mut pending).unwrap();
        if include_provider {
            pending
                .register_external_graph_authorities(&provider)
                .unwrap();
        }
        input.resolve_identities(&mut pending).unwrap();
        pending.finish().unwrap()
    }

    pub(super) fn without_members(&self) -> NativeBoundaryTypeDefinitionRecord {
        let shape = match self.record.shape() {
            NativeBoundaryNominalShape::Struct { c_layout, .. } => {
                NativeBoundaryNominalShape::Struct {
                    c_layout: *c_layout,
                    fields: vec![],
                }
            }
            NativeBoundaryNominalShape::Enum { .. } => {
                NativeBoundaryNominalShape::Enum { variants: vec![] }
            }
            NativeBoundaryNominalShape::Reference => panic!("fixture is a value type"),
        };
        NativeBoundaryTypeDefinitionRecord::new(
            &self.declaration,
            &[self.record.type_parameter_count()],
            shape,
        )
        .unwrap()
    }
}

use super::*;

#[test]
fn metered_source_reader_resolves_real_foundation_keys_and_preserves_legacy_wire() {
    let table = CanonicalExportDefinitionSourcesV1::try_new(vec![source(3), source(1)]).unwrap();
    let decoded = decoded(&table);
    let legacy = decoded.clone().resolve(&mut Foundation::default()).unwrap();
    let mut foundation = Foundation::default();
    let mut resources = meter();
    let result = decoded
        .resolve_metered(&mut foundation, &mut resources, &WirePath::root().field(8))
        .unwrap();
    assert_eq!(result, table);
    assert_eq!(result, legacy);
    assert_eq!(encode(&result).unwrap(), encode(&table).unwrap());
    assert_eq!((foundation.cone_calls, foundation.context_calls), (2, 2));
    let usage = resources.usage();
    assert!(
        usage.decoded_nodes > 0
            && usage.decoded_edges > 0
            && usage.owned_bytes > 0
            && usage.validation_work_units > 0
    );
}
#[test]
fn metered_source_reader_rejects_duplicate_and_reversed_rows_without_repair() {
    for duplicate in [true, false] {
        let input = if duplicate {
            vec![source(1), source(1)]
        } else {
            vec![source(3), source(1)]
        };
        let error = decoded(&Sequence(input))
            .resolve_metered(&mut Foundation::default(), &mut meter(), &WirePath::root())
            .unwrap_err();
        let MeteredDefinitionSourcesResolutionError::Semantic(error) = error else {
            panic!("expected semantic canonicality failure");
        };
        if duplicate {
            assert!(matches!(
                error,
                ExportDefinitionSourceSetValidationError::Duplicate { index: 1 }
            ));
        } else {
            assert!(matches!(
                error,
                ExportDefinitionSourceSetValidationError::NonCanonicalOrder { index: 1 }
            ));
        }
    }
}
#[test]
fn metered_source_reader_retains_source_context_and_origin_identity_errors() {
    let foreign = source_identity(
        ConeCoordinate::new("example", "foreign", "1.0.0")
            .unwrap()
            .identity()
            .unwrap(),
        "foreign.scoop",
    );
    let origin = ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(
            foreign.clone(),
            SourceSpan::new(0, 1).unwrap(),
            &SourceContextKey::File { source: foreign },
        )
        .unwrap(),
    );
    let error = decoded(&Sequence(vec![origin]))
        .resolve_metered(&mut Foundation::default(), &mut meter(), &WirePath::root())
        .unwrap_err();
    assert!(matches!(
        error,
        MeteredDefinitionSourcesResolutionError::Semantic(
            ExportDefinitionSourceSetValidationError::Source {
                index: 0,
                error: SourceOriginResolutionError::Source(SourceIdentityResolutionError::Cone(
                    "unknown foundation cone"
                ))
            }
        )
    ));
    let error = one()
        .resolve_metered(
            &mut Foundation::new("other.scoop"),
            &mut meter(),
            &WirePath::root(),
        )
        .unwrap_err();
    assert!(matches!(
        error,
        MeteredDefinitionSourcesResolutionError::Semantic(
            ExportDefinitionSourceSetValidationError::Source {
                index: 0,
                error: SourceOriginResolutionError::Context("unknown foundation source context")
            }
        )
    ));
    let mut foundation = Foundation::new("other.scoop");
    let error = decoded(&DifferentContext(&foundation.context))
        .resolve_metered(&mut foundation, &mut meter(), &WirePath::root())
        .unwrap_err();
    assert!(matches!(
        error,
        MeteredDefinitionSourcesResolutionError::Semantic(
            ExportDefinitionSourceSetValidationError::Source {
                index: 0,
                error: SourceOriginResolutionError::Origin(
                    SourceOriginError::ContextSourceMismatch
                )
            }
        )
    ));
}

struct DifferentContext<'a>(&'a SourceContextKey);
impl WireEncode for DifferentContext<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(1)?;
        encoder.map(3)?;
        encoder.field(1)?;
        source_identity(ConeIdentity::CORE, SOURCE_PATH).encode(encoder)?;
        encoder.field(2)?;
        SourceSpan::new(1, 2).unwrap().encode(encoder)?;
        encoder.field(3)?;
        PersistentSourceContextId::from_key(self.0)
            .unwrap()
            .encode(encoder)
    }
}

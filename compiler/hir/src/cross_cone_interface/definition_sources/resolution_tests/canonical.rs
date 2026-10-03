use super::*;

#[test]
fn source_reader_rejects_duplicate_and_reversed_rows_without_repair() {
    for duplicate in [true, false] {
        let input = if duplicate {
            vec![source(1), source(1)]
        } else {
            vec![source(3), source(1)]
        };
        let error = decoded(&Sequence(input))
            .resolve(&mut Foundation::default())
            .unwrap_err();
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
fn source_reader_retains_source_context_and_origin_identity_errors() {
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
        .resolve(&mut Foundation::default())
        .unwrap_err();
    assert!(matches!(
        error,
        ExportDefinitionSourceSetValidationError::Source {
            index: 0,
            error: SourceOriginResolutionError::Source(SourceIdentityResolutionError::Cone(
                "unknown foundation cone"
            ))
        }
    ));
    let error = one()
        .resolve(&mut Foundation::new("other.scoop"))
        .unwrap_err();
    assert!(matches!(
        error,
        ExportDefinitionSourceSetValidationError::Source {
            index: 0,
            error: SourceOriginResolutionError::Context("unknown foundation source context")
        }
    ));
    let mut foundation = Foundation::new("other.scoop");
    let error = decoded(&DifferentContext(&foundation.context))
        .resolve(&mut foundation)
        .unwrap_err();
    assert!(matches!(
        error,
        ExportDefinitionSourceSetValidationError::Source {
            index: 0,
            error: SourceOriginResolutionError::Origin(SourceOriginError::ContextSourceMismatch)
        }
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

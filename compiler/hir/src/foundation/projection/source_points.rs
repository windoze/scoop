use super::*;
use scoop_identity::SourceIdentity;

struct RequiredSourcePoints {
    offsets: Vec<u64>,
    first_origin: RequiredSourceOrigin,
}

enum RequiredSourceOrigin {
    Foundation(scoop_identity::DefinitionOriginSubject),
    CrossConeInterface,
}

pub(super) fn source_records(
    source_files: &[crate::SourceFileMetadata],
    definitions: &[DefinitionOriginRecord],
    interface_sources: &[crate::ExportDefinitionSourceV1],
    existing: &[SourceRecord],
) -> Result<Vec<SourceRecord>, HirFoundationBuildError> {
    let mut required = BTreeMap::<SourceIdentity, RequiredSourcePoints>::new();
    for record in existing {
        required.insert(
            record.identity().clone(),
            RequiredSourcePoints {
                offsets: record
                    .points()
                    .iter()
                    .map(crate::SourcePointRecord::byte_offset)
                    .collect(),
                first_origin: RequiredSourceOrigin::CrossConeInterface,
            },
        );
    }
    for record in definitions {
        let origin = record.origin();
        let span = origin.span();
        let points =
            required
                .entry(origin.source().clone())
                .or_insert_with(|| RequiredSourcePoints {
                    offsets: Vec::new(),
                    first_origin: RequiredSourceOrigin::Foundation(record.subject()),
                });
        points.offsets.extend([span.start_byte(), span.end_byte()]);
    }
    for source in interface_sources {
        let origin = source.origin();
        let span = origin.span();
        let points =
            required
                .entry(origin.source().clone())
                .or_insert_with(|| RequiredSourcePoints {
                    offsets: Vec::new(),
                    first_origin: RequiredSourceOrigin::CrossConeInterface,
                });
        points.offsets.extend([span.start_byte(), span.end_byte()]);
    }

    let mut records = Vec::with_capacity(source_files.len());
    for source in source_files {
        let points = required
            .remove(&source.identity)
            .map_or_else(Vec::new, |points| points.offsets);
        let record = if let Some(record) = &source.canonical_record {
            if record.identity() != &source.identity {
                return Err(HirFoundationBuildError::CanonicalSourceIdentityMismatch {
                    metadata: source.identity.clone(),
                    record: record.identity().clone(),
                });
            }
            record.require_points(points).map_err(|error| {
                HirFoundationBuildError::CanonicalSourcePoints {
                    source: source.identity.clone(),
                    error,
                }
            })?;
            record.clone()
        } else {
            SourceRecord::from_utf8(source.identity.clone(), &source.source, points).map_err(
                |error| HirFoundationBuildError::SourceRecord {
                    source: source.identity.clone(),
                    error,
                },
            )?
        };
        records.push(record);
    }
    if let Some((source, points)) = required.into_iter().next() {
        return Err(match points.first_origin {
            RequiredSourceOrigin::Foundation(subject) => {
                HirFoundationBuildError::UnknownDefinitionSource {
                    source,
                    subject_tag: subject.kind_tag(),
                    subject: subject.raw_id(),
                }
            }
            RequiredSourceOrigin::CrossConeInterface => {
                HirFoundationBuildError::UnknownCrossConeDefinitionSource(source)
            }
        });
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use scoop_identity::{
        CoreBuiltinNominal, DefinitionOrigin, DefinitionOriginSubject, SourceContextKey, SourceSpan,
    };

    use super::*;

    #[test]
    fn source_projection_collects_foundation_and_cross_cone_origin_endpoints() {
        let source = SourceIdentity::single_file();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let origin =
            DefinitionOrigin::new(source.clone(), SourceSpan::new(1, 3).unwrap(), &context)
                .unwrap();
        let definition = DefinitionOriginRecord::new(
            DefinitionOriginSubject::Type(CoreBuiltinNominal::Unit.identity_record().id()),
            origin,
        );
        let interface_source = crate::ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 4).unwrap(), &context)
                .unwrap(),
        );
        let metadata = crate::SourceFileMetadata {
            provider: crate::IntrinsicProviderId::from_raw(7),
            identity: source,
            name: "main.scoop".to_string(),
            source: "aé\nz".to_string(),
            canonical_record: None,
        };

        let records = source_records(&[metadata], &[definition], &[interface_source], &[]).unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(
            records[0]
                .points()
                .iter()
                .map(crate::SourcePointRecord::byte_offset)
                .collect::<Vec<_>>(),
            vec![0, 1, 3, 4]
        );
        assert_eq!(records[0].point(1).unwrap().column(), 2);
        assert_eq!(records[0].point(3).unwrap().column(), 3);
    }

    #[test]
    fn source_projection_rejects_an_origin_outside_the_module() {
        let source = SourceIdentity::single_file();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let subject =
            DefinitionOriginSubject::Type(CoreBuiltinNominal::Unit.identity_record().id());
        let origin =
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 0).unwrap(), &context)
                .unwrap();

        assert!(matches!(
            source_records(&[], &[DefinitionOriginRecord::new(subject, origin)], &[], &[]),
            Err(HirFoundationBuildError::UnknownDefinitionSource {
                source: actual_source,
                subject_tag,
                subject: actual_subject,
            }) if actual_source == source
                && subject_tag == subject.kind_tag()
                && actual_subject == subject.raw_id()
        ));
    }

    #[test]
    fn source_completion_preserves_existing_points_and_rejects_sparse_canonical_records() {
        let source = SourceIdentity::single_file();
        let existing = SourceRecord::from_utf8(source.clone(), "abcd", [1, 3]).unwrap();
        let mut metadata = crate::SourceFileMetadata {
            provider: crate::IntrinsicProviderId::from_raw(7),
            identity: source.clone(),
            name: "main.scoop".to_string(),
            source: "abcd".to_string(),
            canonical_record: None,
        };
        let completed = source_records(
            std::slice::from_ref(&metadata),
            &[],
            &[],
            std::slice::from_ref(&existing),
        )
        .unwrap();
        completed[0].require_points([1, 3]).unwrap();
        metadata.canonical_record = Some(SourceRecord::from_utf8(source, "abcd", [0, 4]).unwrap());
        assert!(matches!(source_records(&[metadata], &[], &[], &[existing]),
            Err(HirFoundationBuildError::CanonicalSourcePoints { error, .. }) if error.byte_offset == 1));
    }

    #[test]
    fn source_projection_rejects_a_cross_cone_origin_outside_the_module() {
        let source = SourceIdentity::single_file();
        let context = SourceContextKey::File {
            source: source.clone(),
        };
        let required = crate::ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 0).unwrap(), &context)
                .unwrap(),
        );

        assert!(matches!(
            source_records(&[], &[], &[required], &[]),
            Err(HirFoundationBuildError::UnknownCrossConeDefinitionSource(actual))
                if actual == source
        ));
    }
}

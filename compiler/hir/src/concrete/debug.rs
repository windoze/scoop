//! Coordinate projection of already resolved source identities and declarations.

use std::collections::HashMap;
use std::sync::Arc;

use scoop_identity::{
    CallableMaterialization, CallableTemplateOwner, DefinitionOriginSubject, SourceIdentity,
};
use scoop_source::{DebugOrigin, DebugRecords, LineIndex, PositionEncoding};

#[derive(Clone, Debug, Default)]
pub struct DebugSources {
    files: Vec<DebugSourceFile>,
    indices: HashMap<Arc<SourceIdentity>, usize>,
    definitions: crate::HirExportDefinitionOrigins,
}

#[derive(Clone, Debug)]
struct DebugSourceFile {
    identity: Arc<SourceIdentity>,
    coordinates: SourceCoordinates,
}

#[derive(Clone, Debug)]
enum SourceCoordinates {
    Current(Arc<LineIndex>),
    Imported(crate::SourceRecord),
}

impl DebugSources {
    pub fn new(module: &crate::Module) -> Self {
        let files: Vec<_> = module
            .source_files
            .iter()
            .map(|file| DebugSourceFile {
                identity: Arc::new(file.identity.clone()),
                coordinates: match &file.canonical_record {
                    Some(record) => SourceCoordinates::Imported(record.clone()),
                    None => {
                        SourceCoordinates::Current(Arc::new(LineIndex::new(&file.source, false)))
                    }
                },
            })
            .collect();
        Self {
            indices: files
                .iter()
                .enumerate()
                .map(|(i, file)| (file.identity.clone(), i))
                .collect(),
            files,
            definitions: module.export_definition_origins.clone(),
        }
    }

    pub fn expression(
        &self,
        origin: crate::DefinitionOrigin,
        records: &mut DebugRecords,
    ) -> DebugOrigin {
        if self.files.is_empty() {
            return DebugOrigin::Generated;
        }
        self.position(origin.file as usize, u64::from(origin.span.start), records)
    }

    pub fn in_source(
        &self,
        source: &DebugOrigin,
        offset: u32,
        records: &mut DebugRecords,
    ) -> DebugOrigin {
        match source {
            DebugOrigin::Generated => DebugOrigin::Generated,
            DebugOrigin::Source(location) => {
                self.position(self.indices[&location.source], u64::from(offset), records)
            }
        }
    }

    pub fn function(
        &self,
        function: CallableMaterialization,
        records: &mut DebugRecords,
    ) -> DebugOrigin {
        let subject = match function.template() {
            CallableTemplateOwner::Function(id) => DefinitionOriginSubject::Function(id),
            CallableTemplateOwner::GenericFunction(id) => {
                DefinitionOriginSubject::GenericFunction(id)
            }
            CallableTemplateOwner::Constructor(id) => DefinitionOriginSubject::Constructor(id),
            CallableTemplateOwner::Accessor(id) => DefinitionOriginSubject::PropertyAccessor(id),
            CallableTemplateOwner::Generated(id) => DefinitionOriginSubject::GeneratedCallable(id),
            CallableTemplateOwner::VariantConstructor(_)
            | CallableTemplateOwner::ReleaseHook(_) => return DebugOrigin::Generated,
        };
        match self.definitions.get(subject) {
            Some(record) => {
                let origin = record.origin();
                self.position(
                    self.indices[origin.source()],
                    origin.span().start_byte(),
                    records,
                )
            }
            None => DebugOrigin::Generated,
        }
    }

    fn position(&self, file: usize, offset: u64, records: &mut DebugRecords) -> DebugOrigin {
        let file = &self.files[file];
        let (line, column) = match &file.coordinates {
            SourceCoordinates::Current(index) => {
                let position = index
                    .position(offset as usize, PositionEncoding::Utf32)
                    .expect("source origins point at UTF-8 token boundaries");
                (position.line as u64 + 1, position.column as u64 + 1)
            }
            SourceCoordinates::Imported(record) => match record.point(offset) {
                Some(point) => (point.line(), point.column()),
                None => (
                    record
                        .line_starts()
                        .partition_point(|start| *start <= offset) as u64,
                    0,
                ),
            },
        };
        records.source(
            file.identity.clone(),
            u32::try_from(line).expect("source lines fit u32"),
            u32::try_from(column).expect("source columns fit u32"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_coordinates_preserve_unicode_columns_crlf_and_imported_points() {
        let identity = SourceIdentity::single_file();
        let source = "fun main() {\r\n    val 中文 = 42\r\n}\r\n";
        let offset = source.find("42").unwrap() as u64;
        let current = SourceCoordinates::Current(Arc::new(LineIndex::new(source, false)));
        let imported = SourceCoordinates::Imported(
            crate::SourceRecord::from_utf8(identity.clone(), source, [offset]).unwrap(),
        );
        for coordinates in [current, imported] {
            let sources = DebugSources {
                files: vec![DebugSourceFile {
                    identity: Arc::new(identity.clone()),
                    coordinates,
                }],
                indices: HashMap::from([(Arc::new(identity.clone()), 0)]),
                definitions: Default::default(),
            };
            let DebugOrigin::Source(location) =
                sources.position(0, offset, &mut DebugRecords::default())
            else {
                panic!("source coordinate is available");
            };
            assert_eq!(location.source.as_ref(), &identity);
            assert_eq!((location.line, location.column), (2, 14));
        }
    }
}

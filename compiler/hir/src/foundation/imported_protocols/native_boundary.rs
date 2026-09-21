use super::*;

/// Closed native-boundary witness authority projected from one trusted Core.
///
/// It deliberately contains only the non-generic fundamental source types
/// whose complete M23-3 shape is compiler-owned. Construction is private so
/// an ordinary Cone cannot substitute arbitrary external nominal records.
#[derive(Clone, Debug)]
pub struct ImportedCoreNativeBoundaryTypes {
    records: Vec<NativeBoundaryTypeDefinitionRecord>,
}

impl ImportedCoreNativeBoundaryTypes {
    pub(super) fn import(
        foundation: &ImportedHirFoundation,
        fundamental: &ImportedCoreFundamentalTypeProtocol,
    ) -> Result<Self, CoreNativeBoundaryImportError> {
        let mut records = Vec::with_capacity(IntegerKind::ALL.len() + 2);
        for kind in IntegerKind::ALL {
            records.push(import_core_primitive(
                foundation,
                fundamental.integer(kind).persistent(),
            )?);
        }
        records.push(import_core_primitive(
            foundation,
            fundamental.boolean().persistent(),
        )?);
        records.push(import_core_string(
            foundation,
            fundamental.string().persistent(),
        )?);
        records.sort_by(|left, right| left.owner().compare_sort_key(right.owner()));
        Ok(Self { records })
    }

    pub(crate) fn definition(
        &self,
        owner: NativeBoundaryNominalOwner,
    ) -> Option<&NativeBoundaryTypeDefinitionRecord> {
        self.records
            .binary_search_by(|record| record.owner().compare_sort_key(owner))
            .ok()
            .map(|index| &self.records[index])
    }

    pub fn records(&self) -> &[NativeBoundaryTypeDefinitionRecord] {
        &self.records
    }
}

fn import_core_primitive(
    foundation: &ImportedHirFoundation,
    identity: PersistentTypeId,
) -> Result<NativeBoundaryTypeDefinitionRecord, CoreNativeBoundaryImportError> {
    let declaration = foundation
        .core_source_type_key(identity)
        .ok_or(CoreNativeBoundaryImportError::MissingSourceType(identity))?;
    let actual = declaration.declaration_kind();
    if actual != SourceDeclarationKind::Struct {
        return Err(CoreNativeBoundaryImportError::UnexpectedDeclarationKind {
            identity,
            expected: SourceDeclarationKind::Struct,
            actual,
        });
    }
    let field_count = foundation.core_source_field_count(identity);
    if field_count != 0 {
        return Err(CoreNativeBoundaryImportError::PrimitiveHasFields {
            identity,
            field_count,
        });
    }
    NativeBoundaryTypeDefinitionRecord::new(
        declaration,
        &[0],
        NativeBoundaryNominalShape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
            fields: Vec::new(),
        },
    )
    .map_err(CoreNativeBoundaryImportError::InvalidDefinition)
}

fn import_core_string(
    foundation: &ImportedHirFoundation,
    identity: PersistentTypeId,
) -> Result<NativeBoundaryTypeDefinitionRecord, CoreNativeBoundaryImportError> {
    let declaration = foundation
        .core_source_type_key(identity)
        .ok_or(CoreNativeBoundaryImportError::MissingSourceType(identity))?;
    let actual = declaration.declaration_kind();
    if actual != SourceDeclarationKind::Class {
        return Err(CoreNativeBoundaryImportError::UnexpectedDeclarationKind {
            identity,
            expected: SourceDeclarationKind::Class,
            actual,
        });
    }
    NativeBoundaryTypeDefinitionRecord::new(
        declaration,
        &[0],
        NativeBoundaryNominalShape::Reference,
    )
    .map_err(CoreNativeBoundaryImportError::InvalidDefinition)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreNativeBoundaryImportError {
    MissingSourceType(PersistentTypeId),
    UnexpectedDeclarationKind {
        identity: PersistentTypeId,
        expected: SourceDeclarationKind,
        actual: SourceDeclarationKind,
    },
    PrimitiveHasFields {
        identity: PersistentTypeId,
        field_count: usize,
    },
    InvalidDefinition(NativeBoundaryDefinitionError),
}

impl fmt::Display for CoreNativeBoundaryImportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot import trusted core native-boundary fundamentals: {self:?}"
        )
    }
}

impl std::error::Error for CoreNativeBoundaryImportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidDefinition(error) => Some(error),
            Self::MissingSourceType(_)
            | Self::UnexpectedDeclarationKind { .. }
            | Self::PrimitiveHasFields { .. } => None,
        }
    }
}

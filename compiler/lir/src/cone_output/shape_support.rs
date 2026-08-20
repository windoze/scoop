use std::fmt;

use scoop_identity::{
    ExactTypeKey, GeneratedNominalIdentityError, GeneratedNominalKey, PersistentExactTypeId,
    PersistentLayoutId, PersistentScanId, PersistentTypeId, RepresentationRole, ScanRole,
    SourceDeclarationIdentityError, SourceDeclarationKey, SourceDeclarationKind,
};
use scoop_wire::HashError;

use crate::Module;

/// One exact nominal whose complete non-callable LIR shape has been proven to
/// exist in the sealed strong output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongLirExactShapeMaterialization {
    nominal: PersistentTypeId,
    exact: PersistentExactTypeId,
    value_layout: PersistentLayoutId,
    ref_scan: PersistentScanId,
    type_descriptor: PersistentExactTypeId,
}

impl StrongLirExactShapeMaterialization {
    pub const fn nominal(&self) -> PersistentTypeId {
        self.nominal
    }

    pub const fn exact(&self) -> PersistentExactTypeId {
        self.exact
    }

    pub const fn value_layout(&self) -> PersistentLayoutId {
        self.value_layout
    }

    pub const fn ref_scan(&self) -> PersistentScanId {
        self.ref_scan
    }

    pub const fn type_descriptor(&self) -> PersistentExactTypeId {
        self.type_descriptor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StrongLirBoxedValueMaterialization {
    Available(StrongLirExactShapeMaterialization),
    NotApplicable,
}

/// One local source root and all non-callable LIR shapes that
/// M23-3 requires before the production section can be built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongLirShapeSupportRoot {
    declaration: SourceDeclarationKey,
    source: StrongLirExactShapeMaterialization,
    boxed_value: StrongLirBoxedValueMaterialization,
    coroutine_step: StrongLirExactShapeMaterialization,
    coroutine_slot: StrongLirExactShapeMaterialization,
}

impl StrongLirShapeSupportRoot {
    pub const fn declaration(&self) -> &SourceDeclarationKey {
        &self.declaration
    }

    pub const fn source(&self) -> &StrongLirExactShapeMaterialization {
        &self.source
    }

    pub const fn boxed_value(&self) -> &StrongLirBoxedValueMaterialization {
        &self.boxed_value
    }

    pub const fn coroutine_step(&self) -> &StrongLirExactShapeMaterialization {
        &self.coroutine_step
    }

    pub const fn coroutine_slot(&self) -> &StrongLirExactShapeMaterialization {
        &self.coroutine_slot
    }
}

/// Complete shape plan retained by a sealed strong LIR output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongLirShapeSupportPlan {
    roots: Vec<StrongLirShapeSupportRoot>,
}

impl StrongLirShapeSupportPlan {
    pub(super) fn from_module(
        module: &Module,
        sources: Vec<SourceDeclarationKey>,
    ) -> Result<Self, StrongLirShapeSupportError> {
        let mut roots = Vec::with_capacity(sources.len());
        let mut previous = None;
        for (index, declaration) in sources.into_iter().enumerate() {
            validate_source(index, &declaration, module.cone)?;
            let source = PersistentTypeId::from_source_declaration(&declaration)
                .map_err(|error| StrongLirShapeSupportError::SourceIdentity { index, error })?;
            if previous.is_some_and(|previous| previous >= source) {
                return Err(StrongLirShapeSupportError::NonCanonicalSources {
                    index,
                    previous: previous.expect("the non-canonical branch has a predecessor"),
                    current: source,
                });
            }
            previous = Some(source);

            let source_shape = exact_shape(module, source)?;
            let boxed_value = match declaration.declaration_kind() {
                SourceDeclarationKind::Struct | SourceDeclarationKind::Enum => {
                    StrongLirBoxedValueMaterialization::Available(generated_exact_shape(
                        module,
                        GeneratedNominalKey::BoxedValue {
                            payload: source_shape.exact(),
                        },
                    )?)
                }
                SourceDeclarationKind::Class
                | SourceDeclarationKind::Interface
                | SourceDeclarationKind::Object
                | SourceDeclarationKind::AnnotationClass => {
                    let exact = generated_exact_identity(GeneratedNominalKey::BoxedValue {
                        payload: source_shape.exact(),
                    })?;
                    if module
                        .meta
                        .exact_types
                        .iter()
                        .any(|record| record.id() == exact)
                    {
                        return Err(StrongLirShapeSupportError::UnexpectedBoxedValue(exact));
                    }
                    StrongLirBoxedValueMaterialization::NotApplicable
                }
                _ => {
                    return Err(StrongLirShapeSupportError::InvalidSource { index });
                }
            };
            let coroutine_step = generated_exact_shape(
                module,
                GeneratedNominalKey::CoroutineStep {
                    result: source_shape.exact(),
                },
            )?;
            let coroutine_slot = generated_exact_shape(
                module,
                GeneratedNominalKey::CoroutineSlot {
                    value: source_shape.exact(),
                },
            )?;
            roots.push(StrongLirShapeSupportRoot {
                declaration,
                source: source_shape,
                boxed_value,
                coroutine_step,
                coroutine_slot,
            });
        }
        Ok(Self { roots })
    }

    pub fn roots(&self) -> &[StrongLirShapeSupportRoot] {
        &self.roots
    }

    pub(super) fn source_declarations(&self) -> Vec<SourceDeclarationKey> {
        self.roots()
            .iter()
            .map(|root| root.declaration.clone())
            .collect()
    }
}

fn validate_source(
    index: usize,
    source: &SourceDeclarationKey,
    producer: scoop_identity::ConeIdentity,
) -> Result<(), StrongLirShapeSupportError> {
    if source.origin() != producer
        || !source.declaration_kind().is_nominal()
        || source.duplicate_signature().type_parameter_count() != 0
    {
        return Err(StrongLirShapeSupportError::InvalidSource { index });
    }
    Ok(())
}

fn generated_exact_shape(
    module: &Module,
    key: GeneratedNominalKey,
) -> Result<StrongLirExactShapeMaterialization, StrongLirShapeSupportError> {
    let nominal = PersistentTypeId::from_generated_key(&key)
        .map_err(StrongLirShapeSupportError::GeneratedNominalIdentity)?;
    exact_shape(module, nominal)
}

fn generated_exact_identity(
    key: GeneratedNominalKey,
) -> Result<PersistentExactTypeId, StrongLirShapeSupportError> {
    let nominal = PersistentTypeId::from_generated_key(&key)
        .map_err(StrongLirShapeSupportError::GeneratedNominalIdentity)?;
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal))
        .map_err(StrongLirShapeSupportError::Hash)
}

fn exact_shape(
    module: &Module,
    nominal: PersistentTypeId,
) -> Result<StrongLirExactShapeMaterialization, StrongLirShapeSupportError> {
    let exact = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal))
        .map_err(StrongLirShapeSupportError::Hash)?;
    let exact_record = module
        .meta
        .exact_types
        .iter()
        .find(|record| record.id() == exact)
        .ok_or(StrongLirShapeSupportError::MissingExactType(exact))?;
    if exact_record.key() != &ExactTypeKey::Nominal(nominal) {
        return Err(StrongLirShapeSupportError::ExactTypeMismatch { exact, nominal });
    }

    let layouts = module
        .meta
        .layouts
        .iter()
        .filter_map(|(_, layout)| {
            let key = layout.identity.layout_record().key();
            (key.exact_type() == exact && key.representation() == RepresentationRole::ManagedValue)
                .then_some(&layout.identity)
        })
        .collect::<Vec<_>>();
    let [layout] = layouts.as_slice() else {
        return Err(StrongLirShapeSupportError::ManagedValueLayoutSet {
            exact,
            actual: layouts
                .iter()
                .map(|identity| identity.layout_record().id())
                .collect(),
        });
    };
    if layout.scan_record().key().layout() != layout.layout_record().id()
        || layout.scan_record().key().role() != ScanRole::InlineValue
    {
        return Err(StrongLirShapeSupportError::InvalidInlineScan(exact));
    }

    let descriptors = module
        .meta
        .type_descriptors
        .iter()
        .filter_map(|(_, descriptor)| {
            (descriptor.identity.exact_type() == exact).then_some(descriptor)
        })
        .collect::<Vec<_>>();
    let [descriptor] = descriptors.as_slice() else {
        return Err(StrongLirShapeSupportError::TypeDescriptorSet {
            exact,
            actual: descriptors.len(),
        });
    };
    if descriptor
        .instance_layout
        .layout_record()
        .key()
        .exact_type()
        != exact
    {
        return Err(StrongLirShapeSupportError::DescriptorLayoutMismatch(exact));
    }

    Ok(StrongLirExactShapeMaterialization {
        nominal,
        exact,
        value_layout: layout.layout_record().id(),
        ref_scan: layout.scan_record().id(),
        type_descriptor: descriptor.identity.exact_type(),
    })
}

#[derive(Debug)]
pub enum StrongLirShapeSupportError {
    InvalidSource {
        index: usize,
    },
    NonCanonicalSources {
        index: usize,
        previous: PersistentTypeId,
        current: PersistentTypeId,
    },
    SourceIdentity {
        index: usize,
        error: SourceDeclarationIdentityError,
    },
    GeneratedNominalIdentity(GeneratedNominalIdentityError),
    Hash(HashError),
    MissingExactType(PersistentExactTypeId),
    ExactTypeMismatch {
        exact: PersistentExactTypeId,
        nominal: PersistentTypeId,
    },
    ManagedValueLayoutSet {
        exact: PersistentExactTypeId,
        actual: Vec<PersistentLayoutId>,
    },
    InvalidInlineScan(PersistentExactTypeId),
    TypeDescriptorSet {
        exact: PersistentExactTypeId,
        actual: usize,
    },
    DescriptorLayoutMismatch(PersistentExactTypeId),
    UnexpectedBoxedValue(PersistentExactTypeId),
}

impl fmt::Display for StrongLirShapeSupportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "cannot seal shape-support LIR materialization: {self:?}"
        )
    }
}

impl std::error::Error for StrongLirShapeSupportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::SourceIdentity { error, .. } => Some(error),
            Self::GeneratedNominalIdentity(error) => Some(error),
            Self::Hash(error) => Some(error),
            Self::InvalidSource { .. }
            | Self::NonCanonicalSources { .. }
            | Self::MissingExactType(_)
            | Self::ExactTypeMismatch { .. }
            | Self::ManagedValueLayoutSet { .. }
            | Self::InvalidInlineScan(_)
            | Self::TypeDescriptorSet { .. }
            | Self::DescriptorLayoutMismatch(_)
            | Self::UnexpectedBoxedValue(_) => None,
        }
    }
}

#[cfg(test)]
mod tests;

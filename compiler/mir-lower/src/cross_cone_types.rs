//! Source representation exports from the actual HIR/MIR production pair.

use scoop_hir as hir;
use scoop_identity::{
    ExactTypeKey, PersistentExactTypeId, PersistentTypeId, ValidatedIdentityGraph,
};
use scoop_mir as mir;
use scoop_wire::WirePath;

mod builtins;
mod representation;
mod resources;
use resources::reserve;

/// Combines actual source representations and the sealed finite shape plan.
pub fn lower_type_exports(
    hir: &hir::CrossConeTypeSemanticsSectionV1,
    input: &mir::ConeMirInput,
    identities: &ValidatedIdentityGraph,
) -> Result<mir::CanonicalParamFreeMirTypeExportsV1, SourceMirTypeProductionError> {
    let source = lower_source_type_exports(hir, input, identities)?;
    let finite = mir::CanonicalParamFreeMirTypeExportsV1::from_finite_shape_support(
        input, &source, identities,
    )?;
    let mut records = source.into_records();
    reserve(&mut records, finite.records().len())?;
    records.extend(finite.into_records());

    Ok(mir::CanonicalParamFreeMirTypeExportsV1::try_new(records)?)
}

/// Produces the source and object-backing constituent of the M23-6 type table.
/// The complete section still requires callable, dispatch and shape products.
pub fn lower_source_type_exports(
    hir: &hir::CrossConeTypeSemanticsSectionV1,
    input: &mir::ConeMirInput,
    identities: &ValidatedIdentityGraph,
) -> Result<mir::CanonicalParamFreeMirTypeExportsV1, SourceMirTypeProductionError> {
    let module = input.module();
    let source = hir.representation_support();
    let mut records = Vec::new();
    reserve(&mut records, source.records().len().saturating_mul(2))?;
    let mut produced = 0;
    let authority = mir::MirTypeBridgeAuthority {
        identities,
        foundation: input.foundation(),
    };
    for identity in module.meta.source_exact_types.iter() {
        let ExactTypeKey::Nominal(owner) = identity.identity_record().key() else {
            continue;
        };
        let Some(representation) = source.get(*owner) else {
            if let Some(record) = builtins::project(hir, input, identities, identity, *owner)? {
                reserve(&mut records, 1)?;
                records.push(record);
            }
            continue;
        };
        let exact = identity.identity_record().id();
        let fact = hir
            .exact_facts()
            .get(exact)
            .ok_or(SourceMirTypeProductionError::MissingFacts(exact))?;
        let facts = facts(fact)?;
        let inheritance = bases(hir, exact)?;
        let (shape, backing) = representation::project(module, identity.ty(), representation)?;
        records.push(mir::ParamFreeMirTypeExportV1::try_new(
            authority,
            exact,
            mir::MirTypeOriginV1::SourceNominal(*owner),
            facts,
            shape,
            inheritance,
        )?);
        if let Some((nominal, backing_exact, shape)) = backing {
            records.push(mir::ParamFreeMirTypeExportV1::try_new(
                authority,
                backing_exact,
                mir::MirTypeOriginV1::GeneratedNominal {
                    nominal,
                    role: scoop_identity::GeneratedNominalKey::ObjectBackingClass {
                        object: *owner,
                    },
                },
                facts,
                shape,
                bases(hir, exact)?,
            )?);
        }
        produced += 1;
    }
    if produced != source.records().len() {
        return Err(SourceMirTypeProductionError::IncompleteSurface {
            expected: source.records().len(),
            actual: produced,
        });
    }

    Ok(mir::CanonicalParamFreeMirTypeExportsV1::try_new(records)?)
}

fn facts(
    source: &hir::ExactTypeFactsV1,
) -> Result<mir::MirTypeFactsV1, SourceMirTypeProductionError> {
    let kind = match source.kind() {
        hir::ExactTypeKindV1::Reference => mir::MirValueKindV1::Reference,
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::ZeroSized,
        } => mir::MirValueKindV1::ZeroSizedValue,
        hir::ExactTypeKindV1::Value {
            zst: hir::ZstStatus::NonZero,
        } => mir::MirValueKindV1::NonZeroValue,
    };
    Ok(mir::MirTypeFactsV1::try_new(
        kind,
        match source.gc() {
            hir::ExactTypeGcV1::GcFree => mir::MirGcKindV1::GcFree,
            hir::ExactTypeGcV1::ContainsManagedReferences => {
                mir::MirGcKindV1::ContainsManagedReferences
            }
        },
    )?)
}

fn bases(
    hir: &hir::CrossConeTypeSemanticsSectionV1,
    exact: PersistentExactTypeId,
) -> Result<mir::MirBaseAndInterfacesV1, SourceMirTypeProductionError> {
    let source = hir
        .inheritance()
        .get(exact)
        .ok_or(SourceMirTypeProductionError::MissingInheritance(exact))?
        .edges();
    let mut interfaces = Vec::new();
    reserve(&mut interfaces, source.direct_interfaces().len())?;
    interfaces.extend_from_slice(source.direct_interfaces());
    Ok(mir::MirBaseAndInterfacesV1 {
        base: match source.direct_base() {
            hir::DirectClassBaseV1::NoClassBase => mir::MirBaseClassV1::None,
            hir::DirectClassBaseV1::ClassBase { exact } => mir::MirBaseClassV1::Base(exact),
        },
        interfaces,
    })
}

#[derive(Debug)]
pub enum SourceMirTypeProductionError {
    Resource(scoop_wire::WireError),
    Bridge(mir::MirTypeBridgeError),
    MissingFacts(PersistentExactTypeId),
    MissingInheritance(PersistentExactTypeId),
    MissingFieldType,
    RepresentationMismatch(PersistentTypeId),
    IncompleteSurface { expected: usize, actual: usize },
    Identity(String),
}
impl From<mir::MirTypeBridgeError> for SourceMirTypeProductionError {
    fn from(error: mir::MirTypeBridgeError) -> Self {
        Self::Bridge(error)
    }
}
impl std::fmt::Display for SourceMirTypeProductionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot produce source MIR type exports: {self:?}")
    }
}
impl std::error::Error for SourceMirTypeProductionError {}

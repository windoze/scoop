//! Actual MIR projection of the finite source shape-support materializations.

use super::*;
use crate::{
    ConeMirInput, GeneratedExactTypeLocation, GeneratedNominalShapeRoot,
    StrongBoxedShapeSupportRoot,
};

mod representation;

impl CanonicalParamFreeMirTypeExportsV1 {
    /// Projects finite source support and the actually materialized ODR helpers.
    pub fn from_generated_shapes(
        input: &ConeMirInput,
        sources: &CanonicalParamFreeMirTypeExportsV1,
        identities: &ValidatedIdentityGraph,
    ) -> Result<Self, MirTypeBridgeError> {
        let plan = input.materialization();
        let authority = MirTypeBridgeAuthority {
            identities,
            foundation: input.foundation(),
        };
        let mut records = Vec::new();
        reserve(&mut records, plan.shape_support().len().saturating_mul(3))?;
        for root in plan.shape_support() {
            let exact = root.shape().exact();

            let source = sources
                .get(exact)
                .ok_or(MirTypeBridgeError::MissingShapeSupportSource { exact })?;
            if source.origin() != &MirTypeOriginV1::SourceNominal(root.shape().source()) {
                return Err(MirTypeBridgeError::ExactOriginMismatch { exact });
            }
            let boxed = match root.boxed() {
                StrongBoxedShapeSupportRoot::Available(boxed) => {
                    Some((boxed, GeneratedNominalKey::BoxedValue { payload: exact }))
                }
                StrongBoxedShapeSupportRoot::ReferenceNominalRequiresNoBox => None,
            };
            let helpers = [
                boxed,
                Some((
                    root.coroutine_step(),
                    GeneratedNominalKey::CoroutineStep { result: exact },
                )),
                Some((
                    root.coroutine_slot(),
                    GeneratedNominalKey::CoroutineSlot { value: exact },
                )),
            ];
            for (helper, role) in helpers.into_iter().flatten() {
                let (facts, representation, bases) = representation::project(
                    input,
                    &source.base_and_interfaces().interfaces,
                    helper.location(),
                    &role,
                )?;
                records.push(ParamFreeMirTypeExportV1::try_new(
                    authority,
                    helper.exact(),
                    MirTypeOriginV1::GeneratedNominal {
                        nominal: helper.nominal(),
                        role,
                    },
                    facts,
                    representation,
                    bases,
                )?);
            }
        }

        for root in plan.generated_nominal_shapes() {
            if let GeneratedExactTypeLocation::Context(storage) = root.location() {
                let facts = MirTypeFactsV1::try_new(
                    if storage.role.is_reference() {
                        MirValueKindV1::Reference
                    } else {
                        MirValueKindV1::NonZeroValue
                    },
                    MirGcKindV1::ContainsManagedReferences,
                )?;
                records.push(ParamFreeMirTypeExportV1::try_new(
                    authority,
                    root.exact(),
                    MirTypeOriginV1::GeneratedNominal {
                        nominal: root.nominal(),
                        role: GeneratedNominalKey::TaskContext(storage),
                    },
                    facts,
                    crate::context_type_representation(storage),
                    MirBaseAndInterfacesV1 {
                        base: MirBaseClassV1::None,
                        interfaces: Vec::new(),
                    },
                )?);
                continue;
            }
            let identity = input
                .module()
                .meta
                .generated_exact_types
                .get(root.location())
                .expect("a generated materialization retains its MIR identity");
            let role = identity.nominal_record().key();
            let boxed_slot = matches!(role, GeneratedNominalKey::CoroutineSlot { value }
                if plan.shape_support().iter().any(|root| matches!(root.boxed(),
                    StrongBoxedShapeSupportRoot::Available(boxed) if boxed.exact() == *value)));
            if !matches!(root, GeneratedNominalShapeRoot::Odr { .. }) && !boxed_slot {
                continue;
            }
            let source_exact = match role {
                GeneratedNominalKey::BoxedValue { payload } => *payload,
                GeneratedNominalKey::CoroutineStep { result } => *result,
                GeneratedNominalKey::CoroutineSlot { value } => *value,
                _ => continue,
            };
            let source_key = identities.canonical_key::<_, ExactTypeKey>(source_exact)?;
            let interfaces = match source_key.as_ref() {
                _ if !matches!(role, GeneratedNominalKey::BoxedValue { .. }) => &[],
                ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                    &sources
                        .get(source_exact)
                        .ok_or(MirTypeBridgeError::MissingShapeSupportSource {
                            exact: source_exact,
                        })?
                        .base_and_interfaces()
                        .interfaces[..]
                }
                ExactTypeKey::Tuple(_)
                | ExactTypeKey::Function { .. }
                | ExactTypeKey::RawPointer(_)
                | ExactTypeKey::NativeFunctionPointer { .. } => &[],
            };
            let (facts, representation, bases) =
                representation::project(input, interfaces, root.location(), role)?;
            records.push(ParamFreeMirTypeExportV1::try_new(
                authority,
                root.exact(),
                MirTypeOriginV1::GeneratedNominal {
                    nominal: root.nominal(),
                    role: role.clone(),
                },
                facts,
                representation,
                bases,
            )?);
        }

        Self::try_new(records)
    }
}

fn reserve<T>(values: &mut Vec<T>, count: usize) -> Result<(), MirTypeBridgeError> {
    let path = WirePath::root();

    scoop_wire::allocation::try_reserve(values, count, &path).map_err(MirTypeBridgeError::Resource)
}

//! Mutate machine candidates without changing the checked source declarations.

use super::*;
use scoop_identity::{DependencyCallableDeclarationId as Declaration, ExactCallableSignature};
use scoop_slib::{
    SharedMirSourceCallableComponent as Component, SharedMirSourceCallablePartition as Partition,
    SharedMirSourceCallableValidationError as Error,
};

mod inventory;
mod mutations;
mod signatures;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    foundation: &mir::OdrFreeMirFoundation,
    ordinary: &mir::CrossConeMirBridgeSectionV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    if !name.starts_with("shared-callables-") && name != "shared-accessors-combined" {
        return;
    }
    source
        .with_inheritance_graph(&[], &mut meter(), |graph, _| {
            let replay = Replay {
                source,
                foundation,
                ordinary,
                section,
                graph,
            };
            replay
                .validate(ordinary, section.callables(), &mut meter())
                .unwrap();
            inventory::check(&replay);
            signatures::check(&replay);
            if name.starts_with("shared-callables-") {
                assert!(replay.function("privateHelper").is_some());
            }
            if name == "shared-callables-combined" {
                let declaration = Declaration::Function(replay.function("sharedCallableDeferred").unwrap());
                assert!(ordinary.export(declaration).is_some());
                assert!(section.callables().get(declaration.implementation()).is_none());
                let records = ordinary.exports().iter().filter(|binding| binding.declaration() != declaration).cloned().collect();
                assert!(matches!(replay.reject(&replay.ordinary(records), section.callables().entries().to_vec()),
                    Error::Missing { declaration: actual, partition: Partition::Ordinary } if actual == declaration));
            }
            let mut measured = meter();
            replay
                .validate(ordinary, section.callables(), &mut measured)
                .unwrap();
            let mut shared = scoop_wire::BudgetMeter::new(DecodeLimits {
                validation_work_units: measured.usage().validation_work_units,
                ..DecodeLimits::default()
            });
            replay
                .validate(ordinary, section.callables(), &mut shared)
                .unwrap();
            assert!(
                replay
                    .validate(ordinary, section.callables(), &mut shared)
                    .is_err()
            );
            for limits in [
                DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    logical_heap_bytes: 0,
                    ..DecodeLimits::default()
                },
            ] {
                assert!(
                    replay
                        .validate(
                            ordinary,
                            section.callables(),
                            &mut scoop_wire::BudgetMeter::new(limits)
                        )
                        .is_err()
                );
            }
        })
        .unwrap();
}

struct Replay<'s, 'g> {
    source: hir::CheckedSharedTypeFoundationV1<'s>,
    foundation: &'s mir::OdrFreeMirFoundation,
    ordinary: &'s mir::CrossConeMirBridgeSectionV1,
    section: &'s mir::CrossConeMirTypeBridgeSectionV1<'s>,
    graph: &'g hir::CheckedNominalInheritanceGraphV1<'g>,
}

impl Replay<'_, '_> {
    fn validate(
        &self,
        ordinary: &mir::CrossConeMirBridgeSectionV1,
        bindings: &mir::CanonicalMirCallableBindingsV1,
        meter: &mut scoop_wire::BudgetMeter,
    ) -> Result<(), Error> {
        scoop_slib::validate_shared_mir_source_callables(
            self.source,
            &[],
            self.graph,
            ordinary,
            bindings,
            meter,
        )
    }

    fn reject(
        &self,
        ordinary: &mir::CrossConeMirBridgeSectionV1,
        records: Vec<mir::ParamFreeMirCallableBindingV1>,
    ) -> Error {
        let records = mir::CanonicalMirCallableBindingsV1::try_new(records).unwrap();
        self.validate(ordinary, &records, &mut meter())
            .expect_err("machine candidates must agree with independently retained source metadata")
    }

    fn ordinary(
        &self,
        records: Vec<mir::ParamFreeMirCallableExportV1>,
    ) -> mir::CrossConeMirBridgeSectionV1 {
        mir::CrossConeMirBridgeSectionV1::try_new(
            self.source.provider(),
            self.foundation,
            records,
            self.ordinary.selected().to_vec(),
        )
        .unwrap()
    }

    fn function(&self, name: &str) -> Option<scoop_identity::PersistentFunctionId> {
        self.source.metadata().public.callable_interfaces().all_declarations().find_map(|source| {
            let scoop_identity::CallableTemplateOrigin::Function(id) = source.declaration() else { return None };
            let key = self.source.metadata().identities.canonical_key::<_, scoop_identity::SourceDeclarationKey>(id).unwrap();
            matches!(key.name(), scoop_identity::DeclarationName::Named(actual) if actual.as_str() == name).then_some(id)
        })
    }
}

fn declaration(binding: &mir::ParamFreeMirCallableBindingV1) -> Option<Declaration> {
    match binding.origin() {
        mir::MirCallableOriginV1::Function(id) => Some(Declaration::Function(*id)),
        mir::MirCallableOriginV1::Accessor(id) => Some(Declaration::PropertyAccessor(*id)),
        _ => None,
    }
}

fn component(error: Error, expected: Component, declaration: Declaration) {
    assert!(
        matches!(error, Error::Mismatch { declaration: actual, component }
        if actual == declaration && component == expected),
        "{error:?}"
    );
}

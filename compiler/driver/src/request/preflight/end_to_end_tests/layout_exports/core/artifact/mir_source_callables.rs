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
    foundation: &mir::CanonicalMirFoundation,
    ordinary: &mir::CrossConeMirBridgeSectionV1,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    if !name.starts_with("shared-callables-") && name != "shared-accessors-combined" {
        return;
    }
    let replay = Replay {
        source,
        foundation,
        ordinary,
        section,
    };
    replay.validate(ordinary, section.callables()).unwrap();
    inventory::check(&replay);
    signatures::check(&replay);
    if name.starts_with("shared-callables-") {
        assert!(replay.function("privateHelper").is_some());
    }
    if name == "shared-callables-combined" {
        let declaration = Declaration::Function(replay.function("sharedCallableDeferred").unwrap());
        assert!(ordinary.export(declaration).is_some());
        assert!(
            section
                .callables()
                .get(declaration.implementation())
                .is_none()
        );
        let records = ordinary
            .exports()
            .iter()
            .filter(|binding| binding.declaration() != declaration)
            .cloned()
            .collect();
        assert!(
            matches!(replay.reject(&replay.ordinary(records), section.callables().entries().to_vec()),
                    Error::Missing { declaration: actual, partition: Partition::Ordinary } if actual == declaration)
        );
    }

    replay.validate(ordinary, section.callables()).unwrap();
}

struct Replay<'s> {
    source: hir::CheckedSharedTypeFoundationV1<'s>,
    foundation: &'s mir::CanonicalMirFoundation,
    ordinary: &'s mir::CrossConeMirBridgeSectionV1,
    section: &'s mir::CrossConeMirTypeBridgeSectionV1<'s>,
}

impl Replay<'_> {
    fn validate(
        &self,
        ordinary: &mir::CrossConeMirBridgeSectionV1,
        bindings: &mir::CanonicalMirCallableBindingsV1,
    ) -> Result<(), Error> {
        scoop_slib::validate_shared_mir_source_callables(
            self.source,
            &[],
            ordinary,
            &mir::StrongCallableBridgeSurfaceV1::from_foundation(self.foundation),
            bindings,
        )
    }

    fn reject(
        &self,
        ordinary: &mir::CrossConeMirBridgeSectionV1,
        records: Vec<mir::ParamFreeMirCallableBindingV1>,
    ) -> Error {
        let records = mir::CanonicalMirCallableBindingsV1::try_new(records).unwrap();
        self.validate(ordinary, &records)
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

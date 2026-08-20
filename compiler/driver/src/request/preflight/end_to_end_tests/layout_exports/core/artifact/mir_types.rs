//! Corrupt only MIR constituents while retaining the artifact's checked HIR.

use super::*;
use scoop_identity::PersistentExactTypeId;
use scoop_slib::{SharedMirTypeComponent as Component, SharedMirTypeValidationError as Error};

mod helpers;
mod inventory;
mod representation;

pub(super) fn check(
    name: &str,
    source: hir::CheckedSharedTypeFoundationV1<'_>,
    core: &hir::CoreBootstrapInterfaceSectionV1,
    foundation: &mir::CanonicalMirFoundation,
    section: &mir::CrossConeMirTypeBridgeSectionV1<'_>,
) {
    source
        .with_inheritance_graph(&[], |graph| {
            let replay = Replay {
                source,
                core,
                foundation,
                section,
                graph,
            };
            scoop_slib::validate_shared_mir_type_exports(
                source,
                &[],
                graph,
                core,
                section.types(),
                section.types(),
                section.shape_support(),
            )
            .unwrap();
            inventory::check(&replay);
            representation::check(&replay, name == "shared-mir-combined");
            helpers::check(&replay, name == "shared-dispatch-combined");
        })
        .unwrap();
}

struct Replay<'s, 'g> {
    source: hir::CheckedSharedTypeFoundationV1<'s>,
    core: &'s hir::CoreBootstrapInterfaceSectionV1,
    foundation: &'s mir::CanonicalMirFoundation,
    section: &'s mir::CrossConeMirTypeBridgeSectionV1<'s>,
    graph: &'g hir::CheckedNominalInheritanceGraphV1<'g>,
}

impl Replay<'_, '_> {
    fn reject(&self, records: Vec<mir::ParamFreeMirTypeExportV1>) -> Error {
        self.reject_shapes(records, self.section.shape_support())
    }

    fn reject_shapes(
        &self,
        records: Vec<mir::ParamFreeMirTypeExportV1>,
        shapes: &mir::CanonicalMirShapeSupportsV1,
    ) -> Error {
        let types = mir::CanonicalParamFreeMirTypeExportsV1::try_new(records).unwrap();
        scoop_slib::validate_shared_mir_type_exports(
            self.source,
            &[],
            self.graph,
            self.core,
            &types,
            &types,
            shapes,
        )
        .expect_err("MIR corruption must not agree with unchanged shared HIR declarations")
    }

    fn replace(
        &self,
        record: &mir::ParamFreeMirTypeExportV1,
        representation: mir::MirTypeRepresentationV1,
        facts: mir::MirTypeFactsV1,
        bases: mir::MirBaseAndInterfacesV1,
    ) -> Vec<mir::ParamFreeMirTypeExportV1> {
        let changed = mir::ParamFreeMirTypeExportV1::try_new(
            mir::MirTypeBridgeAuthority {
                identities: self.source.metadata().identities,
                foundation: self.foundation,
            },
            record.exact(),
            record.origin().clone(),
            facts,
            representation,
            bases,
        )
        .expect("the mutation must pass MIR identity and structural validation");
        self.section
            .types()
            .records()
            .iter()
            .map(|record| {
                if record.exact() == changed.exact() {
                    changed.clone()
                } else {
                    record.clone()
                }
            })
            .collect()
    }

    fn reject_representation(
        &self,
        record: &mir::ParamFreeMirTypeExportV1,
        representation: mir::MirTypeRepresentationV1,
    ) -> Error {
        self.reject(self.replace(
            record,
            representation,
            record.facts(),
            record.base_and_interfaces().clone(),
        ))
    }
}

fn component(error: Error, expected: Component, exact: PersistentExactTypeId) {
    assert!(
        matches!(error, Error::Mismatch { component, exact: actual } if component == expected && actual == exact),
        "{error:?}"
    );
}

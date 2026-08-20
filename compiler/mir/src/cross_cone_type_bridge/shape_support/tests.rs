use super::*;
use crate::cross_cone_type_bridge::tests::support::Fixture;
use scoop_identity::SourceNominalKind;
use scoop_wire::{decode_canonical, encode};

mod records;
mod wire;

struct Family {
    fixture: Fixture,
    types: CanonicalParamFreeMirTypeExportsV1,
    boxed: MirBoxedShapeSupportV1,
}
impl Family {
    fn value(scalar: bool) -> Self {
        let fixture = Fixture::new();
        let source = if scalar {
            fixture
                .source(
                    fixture.empty.id(),
                    MirTypeFactsV1::try_new(MirValueKindV1::NonZeroValue, MirGcKindV1::GcFree)
                        .unwrap(),
                    MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Boolean),
                )
                .unwrap()
        } else {
            fixture.empty_export()
        };
        let boxed = MirBoxedShapeSupportV1::Available(fixture.boxed_export().exact());
        let types = CanonicalParamFreeMirTypeExportsV1::try_new(vec![
            source,
            fixture.boxed_export(),
            fixture.step_export(),
            fixture.slot_export(),
        ])
        .unwrap();
        Self {
            fixture,
            types,
            boxed,
        }
    }
    fn reference(valid_gc: bool) -> Self {
        let fixture = Fixture::with_source("Reference", SourceNominalKind::Class);
        let source = fixture
            .source(
                fixture.empty.id(),
                MirTypeFactsV1::try_new(
                    MirValueKindV1::Reference,
                    MirGcKindV1::ContainsManagedReferences,
                )
                .unwrap(),
                MirTypeRepresentationV1::Class {
                    release_policy: Default::default(),
                    kind: MirClassKindV1::Final,
                    declared_fields: vec![],
                },
            )
            .unwrap();
        let (mut step, mut slot) = (fixture.step_export(), fixture.slot_export());
        if valid_gc {
            step = with_reference_payload(&fixture, step, 0);
            slot = with_reference_payload(&fixture, slot, 1);
        }
        let types = CanonicalParamFreeMirTypeExportsV1::try_new(vec![source, step, slot]).unwrap();
        Self {
            fixture,
            types,
            boxed: MirBoxedShapeSupportV1::ReferenceNominalRequiresNoBox,
        }
    }
    fn authority(&self) -> MirShapeSupportAuthority<'_> {
        MirShapeSupportAuthority {
            identities: &self.fixture.graph,
            types: &self.types,
        }
    }
    fn build(&self) -> Result<ParamFreeMirShapeSupportV1, MirShapeSupportError> {
        ParamFreeMirShapeSupportV1::try_new(
            self.authority(),
            self.fixture.empty.id(),
            self.fixture.payload.id(),
            self.boxed,
            self.fixture.step_export().exact(),
            self.fixture.slot_export().exact(),
        )
    }
    fn table(&self) -> CanonicalMirShapeSupportsV1 {
        CanonicalMirShapeSupportsV1::try_new(
            ConeIdentity::SINGLE_FILE,
            self.authority(),
            vec![self.build().unwrap()],
        )
        .unwrap()
    }
}

fn with_reference_payload(
    fixture: &Fixture,
    record: ParamFreeMirTypeExportV1,
    index: usize,
) -> ParamFreeMirTypeExportV1 {
    let mut representation = record.representation().clone();
    let (MirTypeRepresentationV1::CoroutineStep { variants }
    | MirTypeRepresentationV1::CoroutineSlot { variants }) = &mut representation
    else {
        unreachable!()
    };
    variants[index].gc = MirGcKindV1::ContainsManagedReferences;
    ParamFreeMirTypeExportV1::try_new(
        fixture.authority(),
        record.exact(),
        record.origin().clone(),
        MirTypeFactsV1::try_new(
            MirValueKindV1::NonZeroValue,
            MirGcKindV1::ContainsManagedReferences,
        )
        .unwrap(),
        representation,
        record.base_and_interfaces().clone(),
    )
    .unwrap()
}

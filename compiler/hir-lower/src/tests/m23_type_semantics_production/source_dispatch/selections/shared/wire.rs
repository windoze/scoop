use super::*;
use hir::{
    NominalDispatchSelectionError as BuildError,
    NominalDispatchSelectionResolutionError as ResolutionError,
};
use scoop_wire::{Encoder, WireEncode, WireErrorKind};

pub(super) fn verify(
    output: &hir::DependencyHirOutput,
    public: &hir::CrossConeHirInterfaceSectionV1,
) {
    let nominal = public
        .nominal_interfaces()
        .all_records()
        .find(|record| {
            record
                .declaration_details()
                .dispatch_selections()
                .records()
                .len()
                > 1
        })
        .unwrap();
    let choices = nominal.declaration_details().dispatch_selections();
    let records = choices.records();
    let mut identities = super::super::super::super::source_inventory::identity_closure(output);
    let resolve = |values: &[hir::NominalDispatchSelectionV1]| {
        decode_canonical::<DecodedCanonicalNominalDispatchSelectionsV1>(
            &encode(&Sequence(values)).unwrap(),
        )
        .unwrap()
    };
    assert!(matches!(
        resolve(&[records[1], records[0]]).resolve(&mut identities),
        Err(ResolutionError::Order(BuildError::NonCanonicalOrder {
            index: 1
        }))
    ));
    assert!(
        matches!(resolve(&[records[0], records[0]]).resolve(&mut identities),
        Err(ResolutionError::Order(BuildError::Duplicate(slot))) if slot == records[0].slot())
    );
    assert!(
        matches!(CanonicalNominalDispatchSelectionsV1::try_new(vec![records[0], records[0]]),
        Err(BuildError::Duplicate(slot)) if slot == records[0].slot())
    );
    let mut missing = scoop_identity::PendingIdentityValidation::new()
        .finish()
        .unwrap();
    assert!(matches!(
        resolve(records).resolve(&mut missing),
        Err(ResolutionError::Reference(_))
    ));

    let error = decode_canonical::<hir::DecodedNominalDeclarationDetailsV1>(
        &encode(&OldDetails(nominal.declaration_details())).unwrap(),
    )
    .unwrap_err();
    assert!(matches!(
        error.kind(),
        WireErrorKind::InvalidLength {
            expected: 7,
            actual: 6
        }
    ));
}

struct Sequence<'a>(&'a [hir::NominalDispatchSelectionV1]);
impl WireEncode for Sequence<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for record in self.0 {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

struct OldDetails<'a>(&'a hir::NominalDeclarationDetailsV1);
impl WireEncode for OldDetails<'_> {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.0.modality().encode(encoder)?;
        encoder.field(2)?;
        self.0.declared_visibility().encode(encoder)?;
        encoder.field(3)?;
        self.0.constructors().encode(encoder)?;
        encoder.field(4)?;
        self.0.members().encode(encoder)?;
        encoder.field(5)?;
        self.0.children().encode(encoder)?;
        encoder.field(6)?;
        self.0.dispatch_order().encode(encoder)
    }
}

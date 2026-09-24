use scoop_identity::CLayoutOverride;

use super::*;

impl Replay<'_, '_> {
    pub(super) fn value(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
        depth: u64,
    ) -> Result<lir::ExactValueLayoutV1> {
        use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Kind};
        let foundation = self.foundation;
        let value = match source.representation() {
            Kind::Intrinsic(Intrinsic::Unit) => {
                lir::ExactValueLayoutV1::unit(identity, foundation, self.meter)?
            }
            Kind::Intrinsic(Intrinsic::Integer(kind)) => lir::ExactValueLayoutV1::scalar(
                identity,
                lir::ScalarRepresentationKindV1::Integer(integer_kind(*kind)),
                foundation,
                self.meter,
            )?,
            Kind::Intrinsic(Intrinsic::Boolean) => lir::ExactValueLayoutV1::scalar(
                identity,
                lir::ScalarRepresentationKindV1::Boolean,
                foundation,
                self.meter,
            )?,
            Kind::Intrinsic(Intrinsic::String)
            | Kind::Class { .. }
            | Kind::Interface
            | Kind::Object { .. }
            | Kind::ObjectBacking { .. }
            | Kind::BoxedValue { .. } => lir::ExactValueLayoutV1::qualified_pointer(
                identity,
                lir::NichePointerKind::Managed,
                foundation,
                self.meter,
            )?,
            Kind::Struct {
                fields,
                c_layout,
                interior_mutable,
            } => {
                let fields = self.fields(fields, depth)?;
                let fields = fields.inputs(self.meter)?;
                match c_layout {
                    mir::MirTypeCLayoutPolicyV1::Ordinary => {
                        lir::ExactValueLayoutV1::ordinary_struct(
                            identity,
                            *interior_mutable,
                            &fields,
                            foundation,
                            self.meter,
                        )?
                    }
                    mir::MirTypeCLayoutPolicyV1::CLayout(policy) => {
                        self.meter.charge_work(
                            foundation.c_abi_layouts().len() as u64,
                            &WirePath::root(),
                        )?;
                        let contract = foundation
                            .c_abi_layouts()
                            .iter()
                            .find(|layout| layout.layout().exact_type() == source.exact())
                            .ok_or(Error::CLayout(source.exact()))?;
                        if override_bytes(contract.layout().aligned()) != policy.aligned.bytes()
                            || override_bytes(contract.layout().packed()) != policy.packed.bytes()
                        {
                            return Err(Error::CLayout(source.exact()));
                        }
                        lir::ExactValueLayoutV1::c_struct(
                            identity,
                            *interior_mutable,
                            &fields,
                            contract,
                            foundation,
                            self.meter,
                        )?
                    }
                }
            }
            Kind::Enum { variants }
            | Kind::CoroutineStep { variants }
            | Kind::CoroutineSlot { variants } => self.enumeration(identity, variants, depth)?,
        };
        let storage = value.value().storage();
        let zero = storage.nonzero().is_none();
        let managed = storage
            .nonzero()
            .is_some_and(|storage| !matches!(storage.scan().as_ref_scan(), lir::RefScan::None));
        if zero != (source.facts().kind() == mir::MirValueKindV1::ZeroSizedValue)
            || managed != (source.facts().gc() == mir::MirGcKindV1::ContainsManagedReferences)
        {
            return Err(Error::SourceFacts(source.exact()));
        }
        Ok(value)
    }
}

const fn override_bytes(value: CLayoutOverride) -> Option<u8> {
    match value {
        CLayoutOverride::Natural => None,
        CLayoutOverride::Bytes(bytes) => Some(bytes.get()),
    }
}

const fn integer_kind(kind: mir::IntegerKind) -> lir::IntegerKind {
    let signedness = match kind.signedness() {
        mir::IntegerSignedness::Signed => lir::IntegerSignedness::Signed,
        mir::IntegerSignedness::Unsigned => lir::IntegerSignedness::Unsigned,
    };
    let width = match kind.width() {
        mir::IntegerWidth::W8 => lir::IntegerWidth::W8,
        mir::IntegerWidth::W16 => lir::IntegerWidth::W16,
        mir::IntegerWidth::W32 => lir::IntegerWidth::W32,
        mir::IntegerWidth::W64 => lir::IntegerWidth::W64,
    };
    lir::IntegerKind::new(signedness, width)
}

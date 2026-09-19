use scoop_identity::{GeneratedNominalKey, PersistentTypeId};

use super::*;

impl Projection<'_, '_> {
    pub(super) fn instance(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
        depth: u64,
    ) -> Result<lir::ExactInstanceLayoutV1> {
        use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Kind};
        let foundation = self.output.foundation();
        if self.physical_type(source.exact())? == mir::Type::Any {
            return Ok(lir::ExactInstanceLayoutV1::abstract_reference(
                identity, foundation, self.meter,
            )?);
        }
        Ok(match source.representation() {
            Kind::Intrinsic(Intrinsic::String) => {
                lir::ExactInstanceLayoutV1::inline_bytes(identity, foundation, self.meter)?
            }
            Kind::Interface => {
                lir::ExactInstanceLayoutV1::abstract_reference(identity, foundation, self.meter)?
            }
            Kind::Class {
                declared_fields, ..
            }
            | Kind::ObjectBacking { declared_fields } => {
                self.class(identity, source, declared_fields, None, depth)?
            }
            Kind::Object { backing } => {
                let backing_shape = self.shape(*backing)?;
                let Kind::ObjectBacking { declared_fields } = backing_shape.representation() else {
                    return Err(ExactLayoutLoweringError::SourceObject(source.exact()));
                };
                let nominal = backing_shape.origin().nominal();
                let backing = self
                    .identities
                    .canonical_record::<PersistentTypeId, GeneratedNominalKey>(nominal)?;
                self.class(
                    identity,
                    backing_shape,
                    declared_fields,
                    Some(&backing),
                    depth,
                )?
            }
            Kind::BoxedValue { payload } => {
                let payload = self.value_dependency(payload.value, depth)?;
                lir::ExactInstanceLayoutV1::boxed_payload(
                    identity, &payload, foundation, self.meter,
                )?
            }
            Kind::Intrinsic(Intrinsic::Unit | Intrinsic::Integer(_) | Intrinsic::Boolean)
            | Kind::Struct { .. }
            | Kind::Enum { .. }
            | Kind::CoroutineStep { .. }
            | Kind::CoroutineSlot { .. } => {
                let payload = self.value_dependency(source.exact(), depth)?;
                lir::ExactInstanceLayoutV1::boxed_payload(
                    identity, &payload, foundation, self.meter,
                )?
            }
        })
    }

    fn class(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
        fields: &[mir::MirRepresentationFieldV1],
        backing: Option<&CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>>,
        depth: u64,
    ) -> Result<lir::ExactInstanceLayoutV1> {
        let base = match source.base_and_interfaces().base {
            mir::MirBaseClassV1::None => None,
            mir::MirBaseClassV1::Base(exact) => Some(
                self.layout(exact, RepresentationRole::ManagedObject, depth)?
                    .instance_handle()
                    .ok_or(ExactLayoutLoweringError::DependencyKind(exact))?,
            ),
        };
        let base = match &base {
            None => lir::ClassLayoutBaseV1::NoBase,
            Some(base) => lir::ClassLayoutBaseV1::Base(base),
        };
        let fields = self.fields(fields, depth)?;
        let fields = fields.inputs(self.meter)?;
        Ok(match backing {
            Some(backing) => lir::ExactInstanceLayoutV1::object(
                identity,
                backing,
                base,
                &fields,
                self.output.foundation(),
                self.meter,
            )?,
            None => lir::ExactInstanceLayoutV1::class(
                identity,
                base,
                &fields,
                self.output.foundation(),
                self.meter,
            )?,
        })
    }
}

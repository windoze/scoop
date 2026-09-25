use scoop_identity::{GeneratedNominalKey, PersistentTypeId};

use super::*;

impl Projection<'_> {
    pub(super) fn instance(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
    ) -> Result<lir::ExactInstanceLayoutV1> {
        use mir::{MirParamFreeIntrinsicV1 as Intrinsic, MirTypeRepresentationV1 as Kind};
        let foundation = self.output.foundation();
        if self.physical_type(source.exact())? == mir::Type::Any {
            return Ok(lir::ExactInstanceLayoutV1::abstract_reference(
                identity, foundation,
            )?);
        }
        Ok(match source.representation() {
            Kind::Intrinsic(Intrinsic::String) => {
                lir::ExactInstanceLayoutV1::inline_bytes(identity, foundation)?
            }
            Kind::Interface => {
                lir::ExactInstanceLayoutV1::abstract_reference(identity, foundation)?
            }
            Kind::Class {
                declared_fields, ..
            }
            | Kind::ObjectBacking { declared_fields } => {
                self.class(identity, source, declared_fields, None)?
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
                self.class(identity, backing_shape, declared_fields, Some(&backing))?
            }
            Kind::BoxedValue { payload } => {
                let payload = self.value_dependency(payload.value)?;
                lir::ExactInstanceLayoutV1::boxed_payload(identity, &payload, foundation)?
            }
            Kind::Intrinsic(Intrinsic::Unit | Intrinsic::Integer(_) | Intrinsic::Boolean)
            | Kind::Struct { .. }
            | Kind::Enum { .. }
            | Kind::CoroutineStep { .. }
            | Kind::CoroutineSlot { .. } => {
                let payload = self.value_dependency(source.exact())?;
                lir::ExactInstanceLayoutV1::boxed_payload(identity, &payload, foundation)?
            }
        })
    }

    fn class(
        &mut self,
        identity: lir::ExactLayoutIdentityV1,
        source: &mir::ParamFreeMirTypeExportV1,
        fields: &[mir::MirRepresentationFieldV1],
        backing: Option<&CborIdentityRecord<PersistentTypeId, GeneratedNominalKey>>,
    ) -> Result<lir::ExactInstanceLayoutV1> {
        let base = match source.base_and_interfaces().base {
            mir::MirBaseClassV1::None => None,
            mir::MirBaseClassV1::Base(exact) => Some(
                self.layout(exact, RepresentationRole::ManagedObject)?
                    .instance_handle()
                    .ok_or(ExactLayoutLoweringError::DependencyKind(exact))?,
            ),
        };
        let base = match &base {
            None => lir::ClassLayoutBaseV1::NoBase,
            Some(base) => lir::ClassLayoutBaseV1::Base(base),
        };
        let fields = self.fields(fields)?;
        let fields = fields.inputs()?;
        Ok(match backing {
            Some(backing) => lir::ExactInstanceLayoutV1::object(
                identity,
                backing,
                base,
                &fields,
                self.output.foundation(),
            )?,
            None => lir::ExactInstanceLayoutV1::class(
                identity,
                base,
                &fields,
                self.output.foundation(),
            )?,
        })
    }
}

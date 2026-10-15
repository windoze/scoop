use std::sync::Arc;

use scoop_identity::{PersistentIdResolver, ValidatedIdentityGraph};

use super::*;
use crate::{
    ConeLirFoundation, ExternalStrongShapeSubjectV1, LinkDataError, LirTargetProfile,
    ValueLayoutConstituentV1, link_data::link_error,
};

mod instance;
mod value;

pub(crate) struct PreparedLayout {
    identity: ExactLayoutIdentityV1,
    scan: super::super::super::model::ScanBinding,
    body: PreparedBody,
}

enum PreparedBody {
    Value(ValueLayoutConstituentV1, RawValue),
    Instance(crate::TypeInstanceShapeV1, RawInstance),
}

impl DecodedExactLayoutExportV1 {
    pub(crate) fn prepare(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<PreparedLayout, LinkDataError> {
        let semantic = self.semantic;
        let layout = identities.resolve(semantic.layout).map_err(link_error)?;
        let exact = identities.resolve(semantic.exact).map_err(link_error)?;
        let layout = identities
            .canonical_record::<_, scoop_identity::LayoutKey>(layout)
            .map_err(link_error)?;
        if scoop_identity::TargetProfileWireId::refine(
            semantic.target.validate().map_err(link_error)?,
        )
        .map_err(link_error)?
            != target.wire_id()
            || layout.key().target_profile() != &target.wire_id()
            || layout.key().exact_type() != exact
            || layout.key().representation() != semantic.role
        {
            return Err(link_error("layout target or key mismatch"));
        }
        let definition = self.definition.read(identities)?;
        if definition.semantic_id() != layout.id() {
            return Err(link_error("layout definition subject mismatch"));
        }
        let physical = crate::StrongShapeDefinitionRefV1::from_saved(
            ExternalStrongShapeSubjectV1::Layout(layout.id()),
            definition.definition_plan(),
            definition.symbol(),
            foundation,
        )?;
        let identity = ExactLayoutIdentityV1 {
            exact: identities.canonical_record(exact).map_err(link_error)?,
            layout,
            target,
            physical,
            definition,
        };
        let id = identities.resolve(semantic.scan).map_err(link_error)?;
        let scan = super::super::super::model::ScanBinding {
            id,
            physical: crate::StrongShapeDefinitionRefV1::from_foundation(
                ExternalStrongShapeSubjectV1::Scan(id),
                foundation,
            )
            .map_err(link_error)?,
        };
        let body = match semantic.body {
            RawBody::Value {
                storage,
                representation,
            } => PreparedBody::Value(
                ValueLayoutConstituentV1 {
                    layout: identity.layout(),
                    key: identity.layout_key().clone(),
                    target,
                    storage: Arc::new(storage.validate(target).map_err(link_error)?),
                },
                representation,
            ),
            RawBody::Instance {
                shape,
                representation,
            } => PreparedBody::Instance(shape.read(target)?, representation),
        };
        Ok(PreparedLayout {
            identity,
            scan,
            body,
        })
    }
}

impl PreparedLayout {
    pub(crate) fn layout(&self) -> PersistentLayoutId {
        self.identity.layout()
    }

    pub(crate) fn finish(
        self,
        foundation: &ConeLirFoundation,
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExactLayoutExportV1, LinkDataError> {
        let Self {
            identity,
            scan,
            body,
        } = self;
        Ok(match body {
            PreparedBody::Value(value, raw) => {
                let representation = raw.read(&value, foundation, identities)?;
                ExactValueLayoutV1 {
                    identity,
                    value,
                    representation,
                    scan,
                }
                .into()
            }
            PreparedBody::Instance(shape, raw) => {
                let representation = raw.read(&identity, &shape, identities)?;
                ExactInstanceLayoutV1 {
                    identity,
                    shape,
                    representation,
                    scan,
                }
                .into()
            }
        })
    }
}

fn fields(
    raw: Vec<RawNominalField>,
    identities: &mut ValidatedIdentityGraph,
    target: LirTargetProfile,
    extent: u64,
) -> Result<Vec<crate::PlacedFieldStorageV1>, LinkDataError> {
    raw.into_iter()
        .map(|field| {
            Ok(crate::PlacedFieldStorageV1 {
                field: identities.resolve(field.id).map_err(link_error)?,
                storage: field.storage.read(identities, target, extent)?,
                access_alignment: crate::NonZeroPow2::new(field.alignment).map_err(link_error)?,
            })
        })
        .collect()
}

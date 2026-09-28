//! Read existing leaves only when a duplicate definition needs a diagnostic.

use scoop_identity::{OdrMemberId, PersistentSafepointSiteId};
use scoop_wire::Digest256;

use super::{OdrDefinitionDifference, OdrDefinitionMergeError};
use crate::{
    CallableDefinitionFingerprintV1, ObjectDefinitionFingerprintV1, RegistrationFingerprintV1,
    ReplayedLayoutLinkSymbolUsesV1, StackmapRecordFingerprintV1,
};

struct Content {
    lir: Digest256,
    object: ObjectDefinitionFingerprintV1,
    stackmaps: Vec<(PersistentSafepointSiteId, StackmapRecordFingerprintV1)>,
}

pub(super) fn difference(
    first: &ReplayedLayoutLinkSymbolUsesV1,
    second: &ReplayedLayoutLinkSymbolUsesV1,
    member: OdrMemberId,
) -> Result<OdrDefinitionDifference, OdrDefinitionMergeError> {
    let first = content(first, member)?;
    let second = content(second, member)?;
    Ok(if first.lir != second.lir {
        OdrDefinitionDifference::Lir
    } else if first.object != second.object {
        OdrDefinitionDifference::Object
    } else if first.stackmaps != second.stackmaps {
        OdrDefinitionDifference::Stackmap
    } else {
        OdrDefinitionDifference::Definition
    })
}

fn content(
    artifact: &ReplayedLayoutLinkSymbolUsesV1,
    member: OdrMemberId,
) -> Result<Content, OdrDefinitionMergeError> {
    let registrations = artifact
        .final_objects()
        .runtime_images()
        .fingerprint()
        .registrations();
    let callables = registrations.callables();
    for callable in callables.fingerprints() {
        if let CallableDefinitionFingerprintV1::Odr(value) = callable.definition()
            && value.member() == member
        {
            let mut stackmaps = callables
                .body_objects()
                .stackmaps()
                .records()
                .iter()
                .filter_map(|record| {
                    let normalized = record.normalized();
                    let site = normalized.canonical();
                    (site.owner() == callable.body())
                        .then_some((site.site(), normalized.fingerprint()))
                })
                .collect::<Vec<_>>();
            stackmaps.sort_unstable_by_key(|(site, _)| *site);
            return Ok(Content {
                lir: value.lir(),
                object: callable.body_definition(),
                stackmaps,
            });
        }
        if let RegistrationFingerprintV1::Odr(value) = callable.registration()
            && value.member() == member
        {
            return Ok(Content {
                lir: value.lir(),
                object: callable.registration_object(),
                stackmaps: Vec::new(),
            });
        }
    }
    for safepoint in registrations.safepoints().fingerprints() {
        if let RegistrationFingerprintV1::Odr(value) = safepoint.registration()
            && value.member() == member
        {
            return Ok(Content {
                lir: value.lir(),
                object: safepoint.object_definition(),
                stackmaps: vec![(safepoint.site(), safepoint.stackmap())],
            });
        }
    }
    for registration in registrations.types().fingerprints() {
        if let RegistrationFingerprintV1::Odr(value) = registration.registration()
            && value.member() == member
        {
            return Ok(Content {
                lir: value.lir(),
                object: registration.registration_object(),
                stackmaps: Vec::new(),
            });
        }
    }
    for shape in registrations.types().shapes() {
        if shape.member().member() == member {
            return Ok(Content {
                lir: shape.canonical().fingerprint(),
                object: shape.object(),
                stackmaps: Vec::new(),
            });
        }
    }
    for immortal in registrations.immortal_objects().fingerprints() {
        if let crate::ImmortalObjectDefinitionFingerprintV1::Odr(value) = immortal.definition()
            && value.member() == member
        {
            return Ok(Content {
                lir: value.lir(),
                object: immortal.object_definition(),
                stackmaps: Vec::new(),
            });
        }
        if let RegistrationFingerprintV1::Odr(value) = immortal.registration()
            && value.member() == member
        {
            return Ok(Content {
                lir: value.lir(),
                object: immortal.registration_object(),
                stackmaps: Vec::new(),
            });
        }
    }
    Err(OdrDefinitionMergeError::MissingContent {
        provider: artifact.provider(),
        member,
    })
}

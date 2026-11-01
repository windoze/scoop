//! Code fingerprint inputs from the directory produced by the archive builder.
use super::*;

pub(super) fn project_directory_records(
    directory: &[SlibMemberContent],
) -> Result<CodeLinkObjectMemberSetV1, CodeObjectProjectionError> {
    let mut members = Vec::new();
    for record in directory {
        match record.role() {
            SlibMemberRole::LinkObject | SlibMemberRole::NativeObject { .. } => {
                members.push(CodeObjectMemberFingerprintV1 {
                    member: record.id(),
                    fingerprint: record
                        .as_link_member()
                        .expect("LinkObject record")
                        .fingerprint()
                        .map_err(CodeObjectProjectionError::Hash)?,
                });
            }
            SlibMemberRole::ExtensionBlob {
                capability,
                requirement: crate::ExtensionRequirement::Link,
            } => {
                return Err(CodeObjectProjectionError::UnsupportedLinkExtension {
                    member: record.id(),
                    capability: capability.clone(),
                });
            }
            _ => {}
        }
    }
    members.sort_unstable_by_key(|member| member.member);
    Ok(CodeLinkObjectMemberSetV1 { members })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CodeObjectProjectionError {
    UnsupportedLinkExtension {
        member: SlibMemberId,
        capability: CapabilityId,
    },
    Hash(HashError),
}
impl fmt::Display for CodeObjectProjectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Self::UnsupportedLinkExtension { member, capability } = self {
            return write!(
                formatter,
                "member {member}: unsupported Link-required extension {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version()
            );
        }
        write!(formatter, "cannot project code objects: {self:?}")
    }
}
impl std::error::Error for CodeObjectProjectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash(source) => Some(source),
            _ => None,
        }
    }
}

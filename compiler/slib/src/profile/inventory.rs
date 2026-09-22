use super::*;

pub(super) fn validate_inventory<S>(
    view: ArtifactProfileView,
    location: SectionLocation,
    expected: &[CapabilityId],
    sections: &[S],
    capability_of: impl Fn(&S) -> &CapabilityId,
    purpose_of: impl Fn(&S) -> MemberPurposeSet,
) -> Result<(), ArtifactProfileInventoryError> {
    let purpose = view.member_purpose();
    for (index, section) in sections.iter().enumerate() {
        let capability = capability_of(section);
        let incompatible = expected.iter().find(|required| {
            required.namespace() == capability.namespace()
                && required.name() == capability.name()
                && *required != capability
        });
        if let Some(required) = incompatible {
            return Err(
                ArtifactProfileInventoryError::ConflictingCapabilityVersion {
                    location,
                    index,
                    required: Box::new(required.clone()),
                    actual: Box::new(capability.clone()),
                },
            );
        }
        if purpose_of(section).contains(purpose) && !expected.contains(capability) {
            return Err(
                ArtifactProfileInventoryError::UnsupportedRequiredCapability {
                    view,
                    location,
                    index,
                    capability: capability.clone(),
                },
            );
        }
    }
    for capability in expected.iter().filter(|capability| {
        CapabilityContractRegistry::contract(capability)
            .is_some_and(|contract| contract.required_for().contains(purpose))
    }) {
        if !sections
            .iter()
            .any(|section| capability_of(section) == capability)
        {
            return Err(ArtifactProfileInventoryError::MissingRequiredCapability {
                view,
                location,
                capability: capability.clone(),
            });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactProfileView {
    Compile,
    Link,
}

impl ArtifactProfileView {
    pub(super) const fn member_purpose(self) -> MemberPurposeSet {
        match self {
            Self::Compile => MemberPurposeSet::COMPILE,
            Self::Link => MemberPurposeSet::LINK,
        }
    }
}

impl fmt::Display for ArtifactProfileView {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Compile => "Compile",
            Self::Link => "Link",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ArtifactProfileInventoryError {
    ConflictingCapabilityVersion {
        location: SectionLocation,
        index: usize,
        required: Box<CapabilityId>,
        actual: Box<CapabilityId>,
    },
    MissingRequiredCapability {
        view: ArtifactProfileView,
        location: SectionLocation,
        capability: CapabilityId,
    },
    UnsupportedRequiredCapability {
        view: ArtifactProfileView,
        location: SectionLocation,
        index: usize,
        capability: CapabilityId,
    },
}

impl fmt::Display for ArtifactProfileInventoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConflictingCapabilityVersion {
                location,
                index,
                required,
                actual,
            } => write!(
                formatter,
                "{location} section {index} has incompatible {}/{} version {}; profile requires version {} exclusively",
                actual.namespace(),
                actual.name(),
                actual.major_version(),
                required.major_version(),
            ),
            Self::MissingRequiredCapability {
                view,
                location,
                capability,
            } => write!(
                formatter,
                "artifact profile requires {location} capability {}/{}/{} for {view}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
            Self::UnsupportedRequiredCapability {
                view,
                location,
                index,
                capability,
            } => write!(
                formatter,
                "{location} section {index} requires unsupported {view} capability {}/{}/{}",
                capability.namespace(),
                capability.name(),
                capability.major_version(),
            ),
        }
    }
}

impl std::error::Error for ArtifactProfileInventoryError {}

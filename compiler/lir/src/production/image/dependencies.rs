use super::{ConeIdentity, ConeImagePlanBuildError};

pub(super) fn canonical(
    producer: ConeIdentity,
    dependencies: &[ConeIdentity],
) -> Result<Vec<ConeIdentity>, ConeImagePlanBuildError> {
    let mut dependencies = dependencies.to_vec();
    dependencies.sort_unstable();
    if dependencies.binary_search(&producer).is_ok() {
        return Err(ConeImagePlanBuildError::SelfDependency(producer));
    }
    if let Some(pair) = dependencies.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(ConeImagePlanBuildError::DuplicateDependency(pair[0]));
    }
    Ok(dependencies)
}

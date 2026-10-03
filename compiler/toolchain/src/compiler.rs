use scoop_protocol::{ScoopcMachineCapabilityV1, ScoopcProtocolCapabilityV1};
use scoop_slib::IdentityAbiDescriptor;
use scoop_wire::{Digest256, HashError, sha256};

const TOOLCHAIN_DISTRIBUTION_ID_V1: &[u8] = b"scoop-toolchain-development-distribution-v1";
const COMPILER_BUILD_IDENTITY_V1: &[u8] =
    concat!("scoopc-build-v1:", env!("CARGO_PKG_VERSION")).as_bytes();

/// Returns the closed identity expected from the paired `scoopc` binary.
/// Executable content is intentionally separate and is added by orchestration
/// after a stable byte snapshot of the selected binary.
pub fn paired_compiler_machine_capability() -> Result<ScoopcMachineCapabilityV1, HashError> {
    let identity_abi = IdentityAbiDescriptor::current()?.fingerprint()?;
    Ok(ScoopcMachineCapabilityV1::new(
        ScoopcProtocolCapabilityV1::current(),
        sha256(TOOLCHAIN_DISTRIBUTION_ID_V1),
        sha256(COMPILER_BUILD_IDENTITY_V1),
        Digest256::from_array(*identity_abi.as_array()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiler_pairing_identity_is_complete_and_stable() {
        let first = paired_compiler_machine_capability().unwrap();
        let second = paired_compiler_machine_capability().unwrap();
        assert_eq!(first, second);
        assert_eq!(first.protocol(), ScoopcProtocolCapabilityV1::current());
    }
}

use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMembersV1 {
    pub format: u32,
    pub root: HostPathCarrier,
    pub members: Vec<PackageMemberV1>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMemberV1 {
    #[serde(with = "format::coordinate")]
    pub coordinate: ConeCoordinate,
    #[serde(with = "format::Kind")]
    pub kind: RequestedConeKind,
    pub path: String,
    pub directory: HostPathCarrier,
    pub manifest: HostPathCarrier,
    pub selected: bool,
    pub sources: scoop_manifest::SourceSelection,
    pub test_sources: scoop_manifest::SourceSelection,
}

impl PackageMembersV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate::version(self.format)?;
        validate::absolute(&self.root)?;
        let mut keys = std::collections::BTreeSet::new();
        for member in &self.members {
            validate::absolute(&member.directory)?;
            validate::absolute(&member.manifest)?;
            scoop_manifest::ConeRelativePath::new(&member.path).map_err(str::to_owned)?;
            if !keys.insert(DependencyCoordinateKey::from_coordinate(&member.coordinate)) {
                return Err("duplicate workspace member".into());
            }
        }
        Ok(())
    }
}

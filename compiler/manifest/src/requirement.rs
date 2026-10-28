use std::{fmt, str::FromStr};

use semver::{Version, VersionReq};

/// A full-version requirement, distinct from an already selected Cone version.
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct VersionRequirement {
    text: String,
    range: VersionReq,
    exact_terms: Vec<Version>,
}

impl VersionRequirement {
    pub fn matches(&self, version: &Version) -> bool {
        self.range.matches(version) && self.exact_terms.iter().all(|exact| exact == version)
    }

    /// Only a single `=version` is an artifact locator's exact requirement.
    pub fn exact_version(&self) -> Option<&Version> {
        if self.range.comparators.len() == 1 {
            self.exact_terms.first()
        } else {
            None
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl FromStr for VersionRequirement {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut exact_terms = Vec::new();
        let terms: Vec<_> = text.split(',').collect();
        for term in &terms {
            let term = term.trim();
            let (operator, version) = [">=", "<=", "=", ">", "<", "^", "~"]
                .into_iter()
                .find_map(|op| term.strip_prefix(op).map(|version| (op, version.trim())))
                .unwrap_or(("", term));
            if terms.len() > 1 && matches!(operator, "" | "^" | "~") {
                return Err("a requirement intersection must contain comparison operators".into());
            }
            let version = Version::parse(version).map_err(|error| {
                format!("expected a complete major.minor.patch version: {error}")
            })?;
            if !version.build.is_empty() && operator != "=" {
                return Err("build metadata requires an exact =version requirement".into());
            }
            if operator == "=" {
                exact_terms.push(version);
            }
        }
        let range = VersionReq::parse(text).map_err(|error| error.to_string())?;
        Ok(Self {
            text: text.to_owned(),
            range,
            exact_terms,
        })
    }
}

impl TryFrom<String> for VersionRequirement {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<VersionRequirement> for String {
    fn from(value: VersionRequirement) -> Self {
        value.text
    }
}

impl fmt::Display for VersionRequirement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

#[cfg(test)]
mod tests;

use std::{fmt, str::FromStr};

/// A portable registry identity, independent of redirects and local aliases.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RegistryUrl(String);

impl RegistryUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for RegistryUrl {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut url = url::Url::parse(value).map_err(|error| error.to_string())?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err("registry must be an HTTP(S) base URL".into());
        }
        if !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("registry URL must not contain credentials, a query, or a fragment".into());
        }
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        Ok(Self(url.into()))
    }
}

impl TryFrom<String> for RegistryUrl {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<RegistryUrl> for String {
    fn from(value: RegistryUrl) -> Self {
        value.0
    }
}

impl fmt::Display for RegistryUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

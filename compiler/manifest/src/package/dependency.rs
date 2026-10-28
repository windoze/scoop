use serde::Deserialize;
use toml::Spanned;

use super::*;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Details {
    version: Option<String>,
    workspace: Option<bool>,
    path: Option<String>,
    artifact: Option<String>,
    registry: Option<String>,
    git: Option<String>,
    github: Option<String>,
    branch: Option<String>,
    tag: Option<String>,
    rev: Option<String>,
    subdir: Option<String>,
}

pub(super) fn declarations(
    values: raw::DependencyMap,
) -> Result<BTreeMap<DependencyCoordinateKey, DependencyDeclaration>, ManifestParseError> {
    values
        .into_iter()
        .map(|(key, value)| {
            let key = key
                .parse()
                .map_err(|message| invalid(message, value.span()))?;
            Ok((key, declaration(value, false)?))
        })
        .collect()
}

pub(super) fn requests(
    values: raw::DependencyMap,
    overrides: bool,
) -> Result<BTreeMap<DependencyCoordinateKey, DependencyRequest>, ManifestParseError> {
    values
        .into_iter()
        .map(|(key, value)| {
            let key = key
                .parse()
                .map_err(|message| invalid(message, value.span()))?;
            match declaration(value, overrides)? {
                DependencyDeclaration::Request(value) => Ok((key, value)),
                DependencyDeclaration::Workspace { span } => Err(invalid(
                    "workspace dependencies and overrides must declare a value, not inherit one",
                    span,
                )),
            }
        })
        .collect()
}

fn declaration(
    raw: Spanned<toml::Value>,
    overrides: bool,
) -> Result<DependencyDeclaration, ManifestParseError> {
    let span = raw.span();
    let details = match raw.into_inner() {
        toml::Value::String(value) => Details {
            version: Some(value),
            ..Details::default()
        },
        value @ toml::Value::Table(_) => value
            .try_into::<Details>()
            .map_err(|error| invalid(error.to_string(), span.clone()))?,
        _ => {
            return Err(invalid(
                "dependency requires a version string or a table",
                span,
            ));
        }
    };
    let error = |message| invalid(message, span.clone());
    if let Some(workspace) = details.workspace {
        let only_workspace = details.version.is_none()
            && details.path.is_none()
            && details.artifact.is_none()
            && details.registry.is_none()
            && details.git.is_none()
            && details.github.is_none()
            && details.branch.is_none()
            && details.tag.is_none()
            && details.rev.is_none()
            && details.subdir.is_none();
        if !workspace || !only_workspace {
            return Err(error(
                "workspace = true must be the entire dependency declaration".into(),
            ));
        }
        return Ok(DependencyDeclaration::Workspace { span });
    }
    let source_count = [
        &details.path,
        &details.artifact,
        &details.registry,
        &details.git,
        &details.github,
    ]
    .into_iter()
    .filter(|value| value.is_some())
    .count();
    if source_count > 1 {
        return Err(error("a dependency must select only one source".into()));
    }
    if overrides && source_count == 0 {
        return Err(error("an override must specify a source".into()));
    }
    let requirement = details
        .version
        .as_deref()
        .map(str::parse::<VersionRequirement>)
        .transpose()
        .map_err(error)?;
    if details.git.is_none()
        && details.github.is_none()
        && [&details.branch, &details.tag, &details.rev, &details.subdir]
            .iter()
            .any(|value| value.is_some())
    {
        return Err(error(
            "branch, tag, rev, and subdir require a Git source".into(),
        ));
    }
    let source = if let Some(value) = &details.path {
        PackageSource::Path(path(value).map_err(|message| error(message.into()))?)
    } else if let Some(value) = &details.artifact {
        if requirement
            .as_ref()
            .and_then(VersionRequirement::exact_version)
            .is_none()
        {
            return Err(error("artifact requires one exact =version".into()));
        }
        PackageSource::Artifact(path(value).map_err(|message| error(message.into()))?)
    } else if let Some(value) = &details.registry {
        PackageSource::Registry(value.parse().map_err(error)?)
    } else if details.git.is_some() || details.github.is_some() {
        PackageSource::Git(git(details).map_err(error)?)
    } else {
        PackageSource::Default
    };
    if !overrides
        && requirement.is_none()
        && matches!(source, PackageSource::Default | PackageSource::Registry(_))
    {
        return Err(error(
            "registry and system dependencies require a version requirement".into(),
        ));
    }
    Ok(DependencyDeclaration::Request(DependencyRequest {
        requirement,
        source,
        span,
    }))
}

fn git(details: Details) -> Result<GitSource, String> {
    let repository = if let Some(repository) = details.github {
        let parts: Vec<_> = repository.split('/').collect();
        if parts.len() != 2
            || parts.iter().any(|part| {
                part.is_empty()
                    || *part == "."
                    || *part == ".."
                    || part
                        .chars()
                        .any(|c| c.is_whitespace() || c == '\0' || c == ':')
            })
        {
            return Err("github requires owner/repo".into());
        }
        format!("https://github.com/{repository}.git")
    } else {
        details.git.expect("Git source was selected")
    };
    if repository.is_empty() || repository.contains('\0') {
        return Err("Git repository must not be empty or contain NUL".into());
    }
    let mut references = Vec::new();
    for (value, constructor) in [
        (
            details.branch,
            GitReference::Branch as fn(String) -> GitReference,
        ),
        (details.tag, GitReference::Tag),
        (details.rev, GitReference::Revision),
    ] {
        if let Some(value) = value {
            if value.is_empty() || value.contains('\0') {
                return Err("Git reference must not be empty or contain NUL".into());
            }
            references.push(constructor(value));
        }
    }
    if references.len() > 1 {
        return Err("Git dependency must choose at most one of branch, tag, and rev".into());
    }
    let subdir =
        ConeRelativePath::new(details.subdir.as_deref().unwrap_or(".")).map_err(str::to_owned)?;
    Ok(GitSource {
        repository,
        reference: references.pop().unwrap_or(GitReference::DefaultBranch),
        subdir,
    })
}

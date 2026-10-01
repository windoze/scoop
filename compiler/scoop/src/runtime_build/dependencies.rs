use super::*;
use std::collections::BTreeMap;

use scoop_wire::{
    Decoder, Encoder, WireDecode, WireEncode, WireError, domain_separated_cbor_hash, sha256,
};

#[derive(Clone, Debug)]
pub(super) struct Dependencies {
    schema: u64,
    reusable: bool,
    headers: Vec<Header>,
}
#[derive(Clone, Debug)]
struct Header {
    kind: u64,
    path: String,
    digest: Digest256,
}

impl Dependencies {
    pub fn collect(inputs: &inputs::Inputs, runtime: &Path, depfiles: Vec<Option<String>>) -> Self {
        let mut headers = BTreeMap::new();
        let mut reusable = true;
        for depfile in depfiles {
            let Some(paths) = depfile.as_deref().and_then(parse_depfile) else {
                reusable = false;
                continue;
            };
            for path in paths {
                let Ok(path) = std::fs::canonicalize(path) else {
                    reusable = false;
                    continue;
                };
                if path.starts_with(runtime) {
                    continue;
                }
                let selected = path
                    .strip_prefix(&inputs.sdk)
                    .ok()
                    .map(|path| (1, path))
                    .or_else(|| {
                        inputs
                            .resource
                            .as_ref()
                            .and_then(|root| path.strip_prefix(root).ok().map(|path| (2, path)))
                    });
                let Some((kind, relative)) = selected else {
                    reusable = false;
                    continue;
                };
                let Some(relative) = relative.to_str() else {
                    reusable = false;
                    continue;
                };
                let Ok(bytes) = std::fs::read(&path) else {
                    reusable = false;
                    continue;
                };
                headers.insert(
                    (kind, relative.to_owned()),
                    Header {
                        kind,
                        path: relative.to_owned(),
                        digest: sha256(&bytes),
                    },
                );
            }
        }
        Self {
            schema: 1,
            reusable,
            headers: headers.into_values().collect(),
        }
    }

    pub fn is_current(&self, inputs: &inputs::Inputs) -> bool {
        self.schema == 1
            && self.reusable
            && self.headers.iter().all(|header| {
                let root = match header.kind {
                    1 => Some(&inputs.sdk),
                    2 => inputs.resource.as_ref(),
                    _ => None,
                };
                let path = Path::new(&header.path);
                if path.as_os_str().is_empty()
                    || path
                        .components()
                        .any(|part| !matches!(part, std::path::Component::Normal(_)))
                {
                    return false;
                }
                root.and_then(|root| std::fs::read(root.join(path)).ok())
                    .is_some_and(|bytes| sha256(&bytes) == header.digest)
            })
    }

    pub fn input_key(&self, base: Digest256) -> Result<Digest256, RuntimeBuildError> {
        struct Key<'a>(Digest256, &'a Dependencies);
        impl WireEncode for Key<'_> {
            fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
                e.map(2)?;
                e.field(1)?;
                self.0.encode(e)?;
                e.field(2)?;
                self.1.encode(e)
            }
        }
        domain_separated_cbor_hash("scoop-runtime-build-key-v1", &Key(base, self)).map_err(error)
    }
}

fn parse_depfile(input: &str) -> Option<Vec<PathBuf>> {
    // Clang is invoked with the fixed, unescaped target `runtime.o`.
    let input = input.strip_prefix("runtime.o:")?;
    let mut paths = Vec::new();
    let mut word = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next()? {
                '\n' => {}
                '\r' if chars.peek() == Some(&'\n') => {
                    chars.next();
                }
                escaped => word.push(escaped),
            },
            '$' if chars.peek() == Some(&'$') => {
                chars.next();
                word.push('$');
            }
            ch if ch.is_whitespace() => {
                if !word.is_empty() {
                    paths.push(PathBuf::from(std::mem::take(&mut word)));
                }
            }
            ch => word.push(ch),
        }
    }
    if !word.is_empty() {
        paths.push(PathBuf::from(word));
    }
    (!paths.is_empty()).then_some(paths)
}

impl WireEncode for Dependencies {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.map(3)?;
        e.field(1)?;
        e.unsigned(self.schema)?;
        e.field(2)?;
        e.unsigned(u64::from(self.reusable))?;
        e.field(3)?;
        e.array(self.headers.len() as u64)?;
        for header in &self.headers {
            e.map(3)?;
            e.field(1)?;
            e.unsigned(header.kind)?;
            e.field(2)?;
            e.text(&header.path)?;
            e.field(3)?;
            header.digest.encode(e)?;
        }
        Ok(())
    }
}
impl WireDecode for Dependencies {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        d.expect_map(3)?;
        Ok(Self {
            schema: d.field(1, Decoder::unsigned)?,
            reusable: d.field(2, |d| Ok(d.unsigned()? == 1))?,
            headers: d.field(3, |d| {
                d.decode_array(|d, _| {
                    d.expect_map(3)?;
                    Ok(Header {
                        kind: d.field(1, Decoder::unsigned)?,
                        path: d.field(2, |d| Ok(d.text()?.to_owned()))?,
                        digest: d.field(3, Digest256::decode)?,
                    })
                })
            })?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clang_dependencies_preserve_spaces_dollars_and_continuations() {
        assert_eq!(
            parse_depfile("runtime.o: /path/a\\ b.h \\\n /path/$$name.h /path/back\\\\slash.h\n")
                .unwrap(),
            [
                PathBuf::from("/path/a b.h"),
                PathBuf::from("/path/$name.h"),
                PathBuf::from("/path/back\\slash.h")
            ]
        );
        assert!(parse_depfile("wrong-target: /a.h").is_none());
        assert!(parse_depfile("runtime.o: /a\\").is_none());
    }
}

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
    path: String,
    digest: Digest256,
}

impl Dependencies {
    pub fn input_paths(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.headers
            .iter()
            .map(|header| PathBuf::from(&header.path))
    }

    pub fn collect(runtime: &Path, depfiles: Vec<Option<String>>) -> Self {
        let mut headers = BTreeMap::new();
        let mut reusable = true;
        for depfile in depfiles {
            let Some(paths) = depfile.as_deref().and_then(parse_depfile) else {
                reusable = false;
                continue;
            };
            for path in paths {
                // Preserve the consumed spelling: resolving symlinks here would
                // miss changes to an include alias while its old target survives.
                if !path.is_absolute() {
                    reusable = false;
                    continue;
                }
                if path.starts_with(runtime) {
                    continue;
                }
                let Some(locator) = path.to_str() else {
                    reusable = false;
                    continue;
                };
                let Ok(bytes) = std::fs::read(&path) else {
                    reusable = false;
                    continue;
                };
                headers.insert(
                    locator.to_owned(),
                    Header {
                        path: locator.to_owned(),
                        digest: sha256(&bytes),
                    },
                );
            }
        }
        Self {
            schema: 2,
            reusable,
            headers: headers.into_values().collect(),
        }
    }

    pub fn is_current(&self) -> bool {
        self.schema == 2
            && self.reusable
            && self.headers.iter().all(|header| {
                let path = Path::new(&header.path);
                path.is_absolute()
                    && std::fs::read(path).is_ok_and(|bytes| sha256(&bytes) == header.digest)
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
                // Locators stay in the local cache. The runtime input identity
                // includes the consumed bytes, not SDK installation paths.
                let mut digests: Vec<_> =
                    self.1.headers.iter().map(|header| header.digest).collect();
                digests.sort();
                e.array(digests.len() as u64)?;
                for digest in digests {
                    digest.encode(e)?;
                }
                Ok(())
            }
        }
        domain_separated_cbor_hash("scoop-runtime-build-key-v2", &Key(base, self)).map_err(error)
    }
}

fn parse_depfile(input: &str) -> Option<Vec<PathBuf>> {
    scoop_process::parse_make_dependencies(input, "runtime.o")
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
            e.map(2)?;
            e.field(1)?;
            e.text(&header.path)?;
            e.field(2)?;
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
                    d.expect_map(2)?;
                    Ok(Header {
                        path: d.field(1, |d| Ok(d.text()?.to_owned()))?,
                        digest: d.field(2, Digest256::decode)?,
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

//! Directory observations retain names and resolved targets, not excluded file contents.

use super::*;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DirectoryInputV1 {
    pub root: HostPathCarrier,
    pub real_root: HostPathCarrier,
    pub locator: HostPathCarrier,
    pub resolved: HostPathCarrier,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub entries: Vec<DirectoryEntryInputV1>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct DirectoryEntryInputV1 {
    pub path: String,
    pub resolved: HostPathCarrier,
    pub state: DirectoryEntryStateV1,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DirectoryEntryStateV1 {
    Directory,
    Unselected,
    File {
        length: u64,
        modified: (bool, u64, u32),
    },
}

impl WireEncode for DirectoryInputV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(7)?;
        for path in [&self.root, &self.real_root, &self.locator, &self.resolved] {
            path.encode(e)?;
        }
        for patterns in [&self.include, &self.exclude] {
            e.array(patterns.len() as u64)?;
            for pattern in patterns {
                e.text(pattern)?;
            }
        }
        e.array(self.entries.len() as u64)?;
        for entry in &self.entries {
            entry.encode(e)?;
        }
        Ok(())
    }
}

impl WireDecode for DirectoryInputV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        if d.array()? != 7 {
            return Err(invalid(d));
        }
        Ok(Self {
            root: decode_path(d)?,
            real_root: decode_path(d)?,
            locator: decode_path(d)?,
            resolved: decode_path(d)?,
            include: d.decode_array(|d, _| d.owned_text())?,
            exclude: d.decode_array(|d, _| d.owned_text())?,
            entries: d.decode_array(|d, _| DirectoryEntryInputV1::decode(d))?,
        })
    }
}

impl WireEncode for DirectoryEntryInputV1 {
    fn encode(&self, e: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        e.array(3)?;
        e.text(&self.path)?;
        self.resolved.encode(e)?;
        match self.state {
            DirectoryEntryStateV1::Directory => {
                e.array(1)?;
                e.unsigned(1)
            }
            DirectoryEntryStateV1::Unselected => {
                e.array(1)?;
                e.unsigned(2)
            }
            DirectoryEntryStateV1::File { length, modified } => {
                e.array(5)?;
                e.unsigned(3)?;
                e.unsigned(length)?;
                e.unsigned(u64::from(modified.0))?;
                e.unsigned(modified.1)?;
                e.unsigned(u64::from(modified.2))
            }
        }
    }
}

impl WireDecode for DirectoryEntryInputV1 {
    fn decode(d: &mut Decoder<'_>) -> Result<Self, WireError> {
        if d.array()? != 3 {
            return Err(invalid(d));
        }
        let path = d.owned_text()?;
        let resolved = decode_path(d)?;
        let fields = d.array()?;
        let state = match (d.unsigned()?, fields) {
            (1, 1) => DirectoryEntryStateV1::Directory,
            (2, 1) => DirectoryEntryStateV1::Unselected,
            (3, 5) => DirectoryEntryStateV1::File {
                length: d.unsigned()?,
                modified: decode_modified(d)?,
            },
            _ => return Err(invalid(d)),
        };
        Ok(Self {
            path,
            resolved,
            state,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directory_response_retains_selection_targets_and_selected_metadata() {
        let path = |text: &str| HostPathCarrier::from_path(std::path::Path::new(text)).unwrap();
        let input = CompilerInputV1::Directory(DirectoryInputV1 {
            root: path("/cone-alias"),
            real_root: path("/cone"),
            locator: path("/cone/src/link/../assets"),
            resolved: path("/cone/assets"),
            include: vec!["**/*.txt".into()],
            exclude: vec!["draft/**".into()],
            entries: [
                ("dir", DirectoryEntryStateV1::Directory),
                ("draft.bin", DirectoryEntryStateV1::Unselected),
                (
                    "雪.txt",
                    DirectoryEntryStateV1::File {
                        length: 3,
                        modified: (true, 100, 42),
                    },
                ),
            ]
            .into_iter()
            .map(|(name, state)| DirectoryEntryInputV1 {
                path: name.into(),
                resolved: path(&format!("/cone/assets/{name}")),
                state,
            })
            .collect(),
        });
        let response = crate::ScoopcResponseEnvelopeV1::success(
            crate::RequestCorrelationId::from_array([9; 16]),
            crate::ScoopcSuccessV1::new(crate::ScoopcProductV1::Check, vec![], vec![])
                .unwrap()
                .with_compiler_inputs(vec![input]),
        );
        let bytes = crate::encode_response_frame(&response).unwrap();
        assert_eq!(crate::decode_response_frame(&bytes).unwrap(), response);
    }
}

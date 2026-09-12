use std::fmt;

/// A stable path into a wire payload.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct WirePath(Vec<PathSegment>);

impl WirePath {
    pub fn root() -> Self {
        Self::default()
    }

    pub fn field(mut self, field: u32) -> Self {
        self.0.push(PathSegment::Field(field));
        self
    }

    pub fn index(mut self, index: u64) -> Self {
        self.0.push(PathSegment::Index(index));
        self
    }

    pub fn key(mut self, kind: &'static str, bytes: [u8; 32]) -> Self {
        self.0.push(PathSegment::Key { kind, bytes });
        self
    }

    pub fn segments(&self) -> &[PathSegment] {
        &self.0
    }

    pub(crate) fn push(&mut self, segment: PathSegment) {
        self.0.push(segment);
    }

    pub(crate) fn pop(&mut self) {
        self.0.pop();
    }
}

impl fmt::Display for WirePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            return formatter.write_str("$");
        }

        formatter.write_str("$")?;
        for segment in &self.0 {
            match segment {
                PathSegment::Field(field) => write!(formatter, ".{field}")?,
                PathSegment::Index(index) => write!(formatter, "[{index}]")?,
                PathSegment::Key { kind, bytes } => {
                    write!(formatter, "[{kind}:")?;
                    for byte in bytes {
                        write!(formatter, "{byte:02x}")?;
                    }
                    formatter.write_str("]")?;
                }
            }
        }
        Ok(())
    }
}

/// One typed component of a [`WirePath`].
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PathSegment {
    Field(u32),
    Index(u64),
    Key { kind: &'static str, bytes: [u8; 32] },
}

#[cfg(test)]
mod tests {
    use super::{PathSegment, WirePath};

    #[test]
    fn path_display_is_stable() {
        let mut path = WirePath::default();
        path.push(PathSegment::Field(3));
        path.push(PathSegment::Index(5));
        path.push(PathSegment::Key {
            kind: "type",
            bytes: [0xab; 32],
        });

        assert_eq!(
            path.to_string(),
            "$.3[5][type:abababababababababababababababababababababababababababababababab]"
        );
    }
}

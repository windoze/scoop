/// Source-facing declaration information for a machine function.
///
/// The machine callable retains its own identity. These display coordinates
/// travel with the IR and never require codegen to reopen a source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDebugSource {
    pub name: String,
    pub file: String,
    /// One-based source line and Unicode scalar column.
    pub line: u32,
    pub column: u32,
}

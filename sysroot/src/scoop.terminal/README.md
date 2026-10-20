# Scoop terminal

Ordinary Scoop terminal commands, input protocols and platform integration.
The compatibility baseline is crossterm 0.29.0. Production code uses Scoop and
small native platform shims; Rust is used only by the PTY test host.

Public contracts: [terminal specification](../../../docs/specs/SCOOP-TERMINAL-SPEC.md).
Implementation status: [M59 progress](../../../docs/milestone59/PROGRESS.md).

The translated terminal commands and protocol behavior derive from crossterm.
Its MIT license is retained in `LICENSE-crossterm`.

The input parser's uppercase-key classification uses the Unicode 17.0
`Uppercase` property. Its data license is retained in `LICENSE-Unicode`.
To regenerate the small range table, download
[DerivedCoreProperties.txt](https://www.unicode.org/Public/17.0.0/ucd/DerivedCoreProperties.txt)
and run `python3 tools/generate_uppercase.py /path/to/DerivedCoreProperties.txt`
from this Cone directory. No locale-dependent C classification is used.

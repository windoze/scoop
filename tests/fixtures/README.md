# Test fixtures

Per-feature fixtures and cross-feature combination fixtures (see
`AGENTS.md`, "编码准则").

Conventions:

- one directory per feature; combination fixtures under `combo/`;
- negative fixtures (compile-error rules) assert the reported location
  and message;
- golden dumps of each pipeline stage output (HIR / MIR / LIR) use
  snapshot testing (`insta`).

# Test fixtures

Per-feature fixtures and cross-feature combination fixtures (see
`AGENTS.md`, "编码准则").

Conventions:

- one directory per milestone (`m<N>-<topic>/`), negative fixtures
  (compile-error rules) under `errors/` inside each group;
- **each `.scoop` fixture keeps its snapshot next to itself** as
  `<name>.scoop.snap` in the same directory (single merged snapshot:
  rendered diagnostics on compile failure, or stage dumps plus the
  program's stdout / trap stderr on success);
- snapshots are produced with `insta` (regenerate pending snapshots
  with `INSTA_FORCE_PASS=1 cargo test -p scoopc --test fixtures`,
  then review and accept);
- fixtures whose first line is `// EXPECT-TRAP` must compile and then
  abort at runtime; their stderr is snapshotted.

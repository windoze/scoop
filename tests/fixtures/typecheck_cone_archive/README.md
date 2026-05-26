# Retired Archive Fixtures

`typecheck_cone_archive` fixtures were retired in P1-T03 with the old `.cone` archive dependency injection suite. Source-only coverage moved to `tests/fixtures/typecheck_cone/` and `tests/fixtures/run_pass_cone/`.

The remaining archive injector is compatibility coverage for retired fixtures only. Production P10 cone builds use per-cone artifacts (`frontend_import.json`, facts/LIR/type_store payloads, and `objs/*`) instead of `.cone` archive API injection.

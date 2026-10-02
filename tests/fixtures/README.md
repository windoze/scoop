# Scoop 文件测试

每个特性保留独立、组合与 negative 用例，源码通常位于 `m<N>-<topic>/`，
编译错误位于各组的 `errors/`。执行条件统一遵守
[fixture schema 1](../fixture_runner/README.md)，入口为 `python3 tests/run_fixtures.py`。

每项用例选择源码开头的 TOML 注释、相邻 `name.fixture.toml` 或目录中的
`fixture.toml`，明确列出输入、步骤、完整诊断／输出、exit 或 signal。
源码必须属于某项用例的 inputs 或 support；支持文件不单独计作通过。
程序中的历史 `EXPECT-*` 注释不再由 runner 解释，实际条件只来自 TOML。

历史端到端声明位于 `legacy-acceptance/`，通过相对路径使用原源码。
[迁移记录](../../docs/milestone23/stage11/MIGRATION.md)保存原快照到当前条件的对应与修订原因。
正常编译通过 `--emit all` 取得同次编译的 AST／HIR／MIR／LIR，随后运行生成的程序；
编译错误与 warning 使用完整 canonical JSON 期望。native companion 的构建步骤显式声明，
普通运行和 moving-GC 作为同一用例的有序变体。

```sh
# 完整发现与验收；存在未归属源码会报配置错误。
python3 tests/run_fixtures.py --all

# 开发时选择已迁移的历史端到端用例。
python3 tests/run_fixtures.py --suite tests/fixtures/legacy-acceptance

# 显式更新选中的 stage golden，随后审阅 diff 并执行只读验证。
python3 tests/run_fixtures.py --suite tests/fixtures/legacy-acceptance \
  --filter 'legacy-m1-hello-*' --update-snapshots
```

`--all` 禁止过滤和更新期望。`cargo test` 只验证保留的 Rust 单元测试，
不能代替文件测试的正式 CLI 验收。并行度用 `--jobs` 控制；步骤和变体内部保持有序。

# scoop.test 示例

先完成[运行准备](../../../README.md#运行准备)，以下命令在本目录执行。这两个示例是独立的普通 library Cone，生产源码在 `src/`，测试在 `tests/`，测试库只声明在 `[test-dependencies]` 中。

| Cone | 内容 |
| --- | --- |
| [assertions](assertions/Cone.toml) | `assertEquals`、`assertTrue`、`assertFailsWith` 和异常信息 |
| [lifecycle](lifecycle/Cone.toml) | `@BeforeTest`、`@AfterTest`、每个测试的独立实例 |

```sh
scoop test --release assertions
scoop test --release lifecycle
scoop test --release assertions --list
scoop test --release assertions --filter CalculatorTests
```

assertions 的完整文件、运行步骤与预期结果见 `scoop example test/assertions`（[正文](../../../share/scoop/examples/topics/test/assertions.md)）。

lifecycle 有两个通过的测试，每个方法取得由 setup 填入一个元素的 Cart，cleanup 随后清空它。

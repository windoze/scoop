# Regex 实施记录

状态：已完成（2026-10-22）。分支：`codex/pcre2-regex`。依据：[设计](INVESTIGATION.md)、[公开规范](../specs/SCOOP-REGEX-SPEC.md)。用法见 [scoop.regex](../../sysroot/src/scoop.regex/README.md)。

## 阶段

1. 公开合同：已写入独立规范。
2. PCRE2-8 准备、最小查询与捕获：已完成，独立依赖准备与发行接入已提交；支持 Boolean、find/matchAt/matchEntire、组名／编号捕获、标量索引、结果快照及 close/use。
3. 惰性迭代、替换、分割与便利入口：已完成，分别提交。
4. JIT、索引转换与生命周期检查：已完成；JIT 在发布编译对象前尝试，锚定查询保留解释器路径，Boolean 不物化捕获与索引。
5. 三目标交付及相关测试：已完成 Darwin/AArch64、Linux GNU/musl 的依赖准备、生产库、消费者运行和 API 文档检查；完成 Darwin 发行组装与组装目录中的消费者验证。

## 验证边界

先在 Darwin/AArch64 验证，再使用 `nuc12:~/repos/scoop` 完成 GNU/musl。仅运行 regex 本身与实际消费者的 release `scoop.test`；不新增 fixture、底层缺口测试或上游 Unicode 兼容性／完备性测试。未运行 HTTP 组合、无关全量测试、debug 矩阵或 GC stress。原始日志与构建工作区放在仓库外，不使用 `/tmp`。

| 目标 | 库测试 | 预编译库消费者 | PCRE2 与生产 `.slib` | API 文档 |
| --- | --- | --- | --- | --- |
| aarch64-apple-darwin | 19 / 19 | 3 / 3 | 成功 | 成功 |
| x86_64-unknown-linux-gnu | 19 / 19 | 3 / 3 | 成功 | 成功 |
| x86_64-unknown-linux-musl | 19 / 19 | 3 / 3 | 成功 | 成功 |

库测试覆盖参数与选项映射、完整输入和标量起点、捕获与组名快照、空匹配推进、字面／模板／回调替换、分割、异常及关闭行为。独立消费者通过 sysroot 解析预编译 Regex，覆盖公开 API 组合及同一编译模式的并发只读查询。API 文档均使用 `--no-deps --deny-doc-warnings`。

Darwin 的 PCRE2 准备更新复用成功。使用真实发行组装入口收集了 `libpcre2-8.a`、`pcre2.h`、`scoop.regex.slib`、版本清单与 PCRE2/SLJIT 许可，再以组装目录中的命令和 sysroot 运行 3 个消费者测试，全部通过。本次未重建完整工具链或生成完整发行归档。

## 问题与提交

未发现需要修改编译器、runtime 或 FFI 的缺口。发行组装曾把 vendor 根目录中的 `.DS_Store` 当成依赖目录；已修复目录枚举并独立提交 `72d7dbcc0`，随后真实组装及消费者运行成功，未为该底层问题新增测试。

公开合同、依赖准备、查询与捕获、迭代、替换、分割、JIT 和独立消费者按功能分别提交。Scoop 与 C 的生产代码按职责拆分，单文件不超过 120 行。已使用 `cargo clean --profile dev` 清理 6.4 GiB 旧开发产物，保留后续验证使用的 release 工具；日志与构建工作区位于两端仓库旁的 `scoop-work/regex`。

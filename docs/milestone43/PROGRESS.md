# M43 实施记录

依据：[设计](DESIGN.md)。独立 Cone 的 integration-test 已移出本里程碑。

## 实施状态

| 批次 | 状态 | 证据 |
| --- | --- | --- |
| M43-1 断言与测试库 | 断言已完成；测试标记随发现功能交付 | 普通 scoop.test Cone、8 个断言函数、失败数据及 10 个独立 fixture；语言规范第 16 章 |
| M43-2 测试输入与依赖 | 待实施 | |
| M43-3 静态发现与生命周期入口 | 待实施 | |
| M43-4 CLI 与运行报告 | 待实施 | |
| M43-5 产物与平台验收 | 待实施 | |

## 工作约定

- 每个可用功能完成格式化、lint、定向验证后提交；失败与修复按实际记录，不预写通过结论。
- 工作文件及工具临时目录位于仓库 `tmp/m43/`，验证产物位于 `target/m43/`；定期清理已无用途的生成目录。
- 本机先运行受影响验证；最终使用 nuc12 的 `~/repos/scoop` 做 Linux GNU/musl 完整验证，保留实际范围与结果。

## 2026-10-14：普通断言库

- 先补语言规范 16.1～16.2，再以 5 个普通 Scoop 源文件实现断言、失败数据及渲染；最长文件 47 行，没有 compiler/runtime 特判或新依赖。
- 覆盖 Boolean、Equals、引用身份、预期异常、显式失败、SourceLocation 默认实参/显式转发、失败值渲染、渲染异常保留和四个类型约束负例。6 个正例均通过 debug/release，异常、引用身份和渲染另有 moving GC 执行。
- `cargo fmt --all`、`cargo clippy --workspace --all-targets` 与三个 CLI 的构建通过。Darwin 定向验收累计 10 个不同 fixture、16 个变体、34 个成功步骤通过；没有运行无关全量测试。
- 成功来源：[首轮](validation/assertions-darwin-initial.json)中的 Boolean、Equals bound、identity、location、ref bound、Throwable value bound；[修复轮](validation/assertions-darwin-fixes.json)中的 equality、exceptions；[最终补验](validation/assertions-darwin-final.json)中的 rendering、Throwable class bound。前两份保留当时失败，不能单独当作完整通过报告。
- 修复的是用例与文档：接口 override 需显式 public；含 Throwable 的 enum/Option 用模式判断，不能假设结构相等；catch 沿既有规则物化 managed 副本，用字段和引用成员验证保留；f-string 插值使用 `${...}`。断言库实现不需要底层绕行。
- 清理旧 `target/m39`、`target/m41`、`target/m42` 生成目录；保留当前构建缓存。nuc12 工作区干净，Rust、LLVM 22.1 与 musl 工具存在，非交互 SSH 需显式配置 Cargo/LLVM PATH；尚未同步或执行 Linux 验收。

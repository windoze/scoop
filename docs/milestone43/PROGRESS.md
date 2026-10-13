# M43 实施记录

依据：[设计](DESIGN.md)。独立 Cone 的 integration-test 已移出本里程碑。

## 实施状态

| 批次 | 状态 | 证据 |
| --- | --- | --- |
| M43-1 断言与测试库 | 断言已完成；测试标记随发现功能交付 | 普通 scoop.test Cone、8 个断言函数、失败数据及 10 个独立 fixture；语言规范第 16 章 |
| M43-2 测试输入与依赖 | manifest 与生产隔离已完成；测试构建接入待实施 | test.sources/test-dependencies 解码及精确诊断；生产源码、依赖图与缓存不受测试输入影响 |
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

## 2026-10-14：测试 manifest 与生产构建隔离

- 先补语言规范 12.2.4 与实现规范 2.7，再接入 `test.sources`、`test-dependencies` 的解码；生产与测试依赖复用相同的 coordinate/locator/span 解析逻辑。测试配置独立于生产语义投影，不增加产物字段或改变 wire 版本。
- 测试依赖重复声明生产依赖时直接诊断测试声明；覆盖相同版本、覆盖版本/locator、未知字段、非法清单/路径/平台谓词、非法 coordinate 和冲突/空 locator。
- 真实生产构建组合证明：根与依赖 Cone 的测试代码均不解析；无效 UTF-8、不可用的测试源码/产物/search-root 依赖、测试反向依赖均不影响普通 build/run。改变测试文件与配置后，生产产物指纹不变，编译器子进程调用列表为空。
- 格式化、workspace clippy、三个 CLI 构建通过；`scoop-manifest` 49 项 Rust 测试及 fixture runner 47 项公共规则通过。Darwin 新增 12 项 fixture 通过（[生产隔离](validation/manifest-production-darwin.json)、[11 项负例](validation/manifest-negative-darwin.json)），另通过 [32 项既有源码选择负例](validation/manifest-regressions-darwin.json)。没有重复断言库或无关全量验证。
- 依赖解析拆入 111 行子模块，manifest 主解析文件降至 177 行；测试配置结构为 42 行。下一步仍需测试用途的源码/依赖合并、定义处名称边界，再交付发现与合成入口；本次不宣称 `scoop test` 已可用。

# M43 实施记录

依据：[设计](DESIGN.md)。独立 Cone 的 integration-test 已移出本里程碑。

## 实施状态

| 批次 | 状态 | 证据 |
| --- | --- | --- |
| M43-1 断言与测试库 | 断言已完成；测试标记随发现功能交付 | 普通 scoop.test Cone、8 个断言函数、失败数据及 10 个独立 fixture；语言规范第 16 章 |
| M43-2 测试输入与依赖 | manifest、源码选择与定义处声明／依赖候选隔离已完成；测试用途编排与依赖接入待实施 | test.sources/test-dependencies 解码；源码角色贯穿发现、解析与前端；生产名称查找排除测试输入，生产构建与缓存不受测试配置影响 |
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

## 2026-10-14：源码角色与同 Cone 声明候选

- 先补语言规范 12.2.4 和实现规范 2.7，再复用现有源码发现流程合并生产／测试文件；默认测试目录可缺失，测试集合可为空，生产集合仍须非空。跨角色的目录重叠、重复文件、symlink 别名／损坏／逃逸均按实际输入检查。
- SourceRole 随发现结果、AST 和快照保存，不改变 SourceIdentity，也不从目录名推断。合并集合按完整路径排序；driver 的解析适配保留角色和原文件诊断，HIR 前端据此收集定义处候选。
- 生产源码的类型、值、函数、扩展、const、别名、注解和 import 排除测试声明；测试专属 package 不遮蔽生产对象限定名。普通调用、泛型正文和默认实参的重载选择不受测试重载改变，测试源码仍能访问生产 internal，不能绕过文件／成员 private。
- 验证时发现 M42 新增的 Float/Double.toBits 块正文不适用于既有精简数值签名测试辅助代码。按已有 IEEE 比较／codec 的分工排除该普通方法，原始实现仍由完整 sysroot fixture 验证；独立修复提交 `36fa9d24`，22 项 imported-core 回归通过。
- 格式化、workspace clippy 及三个 CLI 构建通过。Rust 验证：manifest 56、AST 15、parser 506 及 1 项 doctest、driver 1 项发现到解析测试；前端新增 7 项组合测试、430 项 M23 回归和 72 项 import 专测通过。没有运行 workspace 全量测试。
- Darwin 正式 CLI 的 M43 manifest、M33 source selection 和跨 Cone qualified-types 回归全部通过：51 个 fixture、52 个变体、100 个进程步骤；包含缓存失效边界、生产构建隔离、准确负例、无源码依赖产物消费、链接与 moving GC 运行，见[报告](validation/source-roles-darwin.json)。
- 源码发现拆出 72 行收集模块；package／源码作用域集中于 140 行模块，namespace 主文件降至 407 行。本轮清理已归档证据的 `target/m43/assertions` 生成目录（约 216 MB）。
- 测试用途的 CLI／子编译器请求、测试依赖合并与定义处依赖候选、测试构建键、合成入口及运行器仍未交付；上述规则的正式 `scoop test` 文件 fixture 随命令接入补齐，当前不宣称 M43-2 或 M43 完成。

## 2026-10-14：定义处依赖候选隔离

- 先补语言规范 12.2.4 和实现规范 2.7，明确按当前 Cone 的直接依赖边筛选公开 binding；前端输入显式携带测试依赖身份，core 保持生产依赖。复用同一依赖语义数据，不分别加载或编译生产与测试依赖闭包。
- package 前缀、当前 package、exact／alias／star import、qualified type 和扩展候选均在冲突与重载选择前筛选；泛型正文和默认实参沿定义处规则。qualified-type 索引保留每个 provider 的贡献，筛选后才合并同一实体的 re-export 路由。
- 生产依赖 re-export 的实体仍可用，即使原始声明 Cone 同时被直接列为测试依赖；选中 nominal 后按普通规则访问其静态成员。新增 6 项依赖角色组合测试与 1 项 re-export 回归，并扩展 HIR namespace 测试以验证同一实体的路由筛选和静态成员访问。
- 格式化、workspace clippy、三个 CLI 构建通过。14 项 M43 前端测试、431 项 M23 回归、72 项 import 专测及 3 项 HIR namespace 测试通过；M23 组包含上述新增 re-export 回归。测试准备中修正了非法同名 property/function、包限定表达式和未公开构造器，未为测试改变语言行为。
- Darwin 正式 CLI 的 7 项跨 Cone 定向 fixture 全部通过，合计 7 个变体、60 个进程步骤；覆盖包前缀、冲突、support package 不可见、diamond re-export、泛型嵌套类型／别名、无源码产物消费及 moving GC 运行，见[报告](validation/dependency-roles-darwin.json)。没有运行无关全量测试。
- `scoop.std` 已明确为首个实际测试应用，设计变更独立提交 `62c1cb3b`。清理已归档报告对应的 `target/m43/source-roles`（约 593 MB），保留当前构建缓存。
- 测试构建请求、根 Cone 的测试依赖图合并、测试构建键与输出、静态目录和生成入口仍待接入；当前仅完成共享前端的依赖名称边界，正式 `scoop test` 和标准库应用尚未交付。

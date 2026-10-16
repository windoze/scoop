# M50 验收

状态：已完成。验收日期：2026-10-17。实现至 `906c7c4f1`，测试期望修正至 `74e367179`。范围见 [设计](DESIGN.md)，分批提交与定向验证见 [实施记录](PROGRESS.md)。

## 实施结果

- 清理自产 IR、对象和产物的完整语义／布局重放、生产凭证、来源比对、只读封存、runtime 静态 metadata 复核及其专属 API、测试和辅助依赖。三份规范与相关历史设计同步修订。
- 保留实际源码语义、typed 引用、对象读取边界、ODR 选择、ABI／GC 数据生成、运行期内存访问和动态 heap 验证；构建缓存继续按输入变化直接复用。
- 恢复普通 COMMON、强弱符号、native 初始化、include 搜索目录和 archive 成员行为。工具链首次探测不再读回 executable 或比对 unwind map。Darwin 动态库不再因 `___cxa_*`、`___gxx_personality`、`___gcc_personality`、内部 weak binding 或 flat namespace 被额外拒绝。
- 新增独立与组合 fixture，验证普通 archive 成员、COMMON、初始化、C++ 异常／C cleanup、跨产物链接及搬迁 GC；异常在 native 动态库内部处理。

## 验证结果

工具均使用 release 构建。Darwin 为本机 aarch64、LLVM 22.1.8；Linux 为 nuc12 的 x86_64、LLVM 22.1.2，使用 GNU 与 musl 工具链。每批代码先格式化与 workspace clippy，再执行定向测试；最终两端 lint 均通过。

| 范围 | 结果 |
| --- | --- |
| Linux Rust workspace，含文档测试 | 4,956 项覆盖完成，首轮唯一失败修正后定向补验通过；无忽略项 |
| 公共 fixture runner | 本机与 Linux 各 50 项通过 |
| GNU 正式 CLI 全目录 | 4,160 项通过，30 项目标不适用；4,701 变体、16,447 进程、70 个专门 golden |
| musl 目标专项 | 88 项通过，21 项目标不适用；113 变体、465 进程、35 个专门 golden |
| Darwin 动态库回归 | 新增 C++／C unwind 与既有 dynamic／C ABI 合计 11 项通过 |
| Darwin 主体清理后功能组合 | 7 项、9 变体、44 进程、4 个链接 plan golden 通过 |
| 最终收尾补验，含失败闭合 | Darwin 6 项、GNU 26 项、musl 4 项 CLI，以及 Linux 4 项 Rust 定向全部通过 |

GNU 完整目录分段执行，调整并发时以已有报告只续跑未完成用例，复用仍然存在的源码与缓存。最终探测清理和期望修正只补验受影响用例，没有重复全量测试。musl 专项覆盖 native／archive／DSO、静态与 PIE、runtime／GC、COMMON／初始化、Scoop ABI 与 callback 等本次受影响路径。Darwin 使用逐批定向覆盖，musl 使用专项覆盖，不将二者记为全目录验收。

## 失败闭合与性能

- ELF metadata 测试包含两个 lazy 单元，迁移时误加的 eager gateway 槽预期已改为无槽；相关 3 项定向复测通过。
- 移除 `/proc/self/maps` 读取后不再导入 `getline`、`sscanf`、`feof`，GNU 还随 Mbed TLS 退役移除 `explicit_bzero`，同步 40 个 GNU／musl 链接计划的实际导入数量。musl 首轮 12 项与 GNU 首轮 19 项数量差异均定向补验通过。实体选择、ABI 与运行断言保留。
- 截断 archive 仍由普通 reader 报错；三个目标诊断期望同步，Darwin 对应用例补验通过。Darwin 两个新增动态库 fixture 在旧 CLI 上复现前缀拒绝，修复后通过。
- 主体清理后，同机三轮冷／暖复测共 24 次 build 通过；四组冷构建中位数降低 29.8%～35.6%，全部暖构建零编译子进程。方法与限制见 [性能记录](PERFORMANCE.md#5-m50-清理后的同机复测)。

原始日志、逐次 JSON、性能报告和工作目录保留在两端 `~/.cache/scoop-m50`，不纳入仓库。全过程按批清理 debug target，保留验证使用的 release 工具；未使用 `/tmp`。无关的 M47、M51 文档改动未纳入提交。

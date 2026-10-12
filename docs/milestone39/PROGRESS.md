# M39 实施记录

基线：`e88f7d73`（仓库历史压缩后的实际 HEAD），2026-10-12 开始。工作区初始干净，无 target 目录。

## 顺序与当前状态

1. 已同步三份规范与路线图：明确布局／签名准备、codegen ABI 规划、完整 LIR 完成，typed 身份与消费者支持边界，工具执行及 runtime 组件职责。
2. 已完成：C toolchain 宿主 invocation 移出 LIR，generated-C 编译交由 driver 编排。
3. 待实施：目标身份／目录／LLVM 配置拆分，全部 Scoop/C ABI 规划迁移。
4. runtime 组件选择已完成；待实施：对象格式／机器规则与 native 工具链目标选择。
5. 待验收：本机定向回归、Linux Rust 与正式 CLI 完整覆盖、边界审查及清理。

## 验证环境与纪律

- 本机使用 LLVM 22.1：`/opt/homebrew/opt/llvm@22`；临时文件与日志放在 `target/m39/`，设置 TMPDIR 指向该目录下的 tmp。
- Linux 环境为 `nuc12:~/repos/scoop`。用户已明确说明远端无有用文件，可直接重置、同步及清理；保留现有 native unwind 安装以避免重复构建。
- 每批 Rust 变更先 fmt、workspace clippy，再定向测试；正式 CLI 复用已有真实功能组合，避免重复全量测试。
- 当前仅记录历史验收为参考：M40 三 target 完整 fixture 已通过；本次尚未执行的测试不计为通过。

## 功能 1：C toolchain invocation 与编译执行归属

- `ValidatedCBridgeToolchainInvocation`、宿主路径检查、命令与环境移入 toolchain；按 invocation、command、tests 拆分，单文件最大 257 行。
- driver 持有 generated-C 的实际编译动作；codegen 复用现有发射流程渲染源码、完成对象边界，不依赖 toolchain 实现 crate。LIR 只保存持久化合同。
- 已通过本机 fmt、workspace clippy；工具链 7 项、generated-C 对象 4 项、持久化合同与固定指纹 6 项测试；debug/release 三个 CLI 均构建成功。
- 真实 CLI 选择 errno 独立／组合／跨 Cone、callback 及 global/TLS，共 24 项；本机全部通过（34 变体、132 进程、10 次 golden 检查），没有更新期望；报告见 [01-darwin.json](validation/01-darwin.json)。
- 清理首次构建的 `target/debug/incremental`，释放约 6.20 GiB；保留编译二进制、依赖库、私有 fixture cache 与日志。

## 功能 2：runtime 的 OS、image 与 arch 组件

- 目标构建按实际 OS、image、arch 组件组合源码、flags 和 includes；源码清单与构建选择分别为 109／92 行。
- 浮点环境初始化移到两个现有架构的专用 C 文件；通用 `floating.c` 不含架构宏或机器寄存器操作。`-mno-red-zone` 属于 x86-64 组件，ELF visibility／SHA 配置属于 image 组件。
- 保留原 `scoop_float_init_environment` 入口与 fesetenv／FPCR／MXCSR 行为，不改公共 runtime ABI。移植后的纯 frame decoder 仍能在其他宿主运行结构测试。
- 本机 fmt、workspace clippy、22 项 toolchain 测试、29 项 runtime collector／线程／初始化回归通过；release CLI 构建成功。
- 浮点环境扰动恢复、值类型、FFI、协程、JSON 及跨 Cone 6 项 CLI 全部通过（9 变体、54 进程），见 [02-darwin.json](validation/02-darwin.json)。无快照更新。

## 功能 3：消费提供方的物理 ABI

- 外部 callable 直接将提供方的 canonical ABI 绑定到消费者本地存储与类型，不再先重新分类后比较；保留参数数量、尺寸／对齐、carrier 范围及既有 exact signature、GC root 协议校验。
- 删除重复的 ABI 比较模块；存储绑定与测试分开。新增隐藏 sret 导致 SysV aggregate 整组回退、后续浮点参数和短 carrier 的精确测试，并覆盖错误存储诊断。
- 本机 fmt、workspace clippy、476 项 LIR 测试、6 项 lowering 外部调用测试、3 项 codegen 外部调用测试通过；release CLI 构建成功。
- 小值跨库产物、suspend member、泛型 release 及 native aggregate release 四项正式 CLI 通过（5 变体、40 进程、4 次 golden 检查），无期望更新，见 [03-darwin.json](validation/03-darwin.json)。

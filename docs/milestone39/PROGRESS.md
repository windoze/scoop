# M39 实施记录

基线：`e88f7d73`（仓库历史压缩后的实际 HEAD），2026-10-12 开始。工作区初始干净，无 target 目录。

## 顺序与当前状态

1. 已同步三份规范与路线图：明确布局／签名准备、codegen ABI 规划、完整 LIR 完成，typed 身份与消费者支持边界，工具执行及 runtime 组件职责。
2. 已完成：C toolchain 宿主 invocation 移出 LIR，generated-C 编译交由 driver 编排。
3. 待实施：目标身份／目录／LLVM 配置拆分，全部 Scoop/C ABI 规划迁移。
4. 待实施：对象格式／机器规则、工具链与 runtime 组件选择。
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

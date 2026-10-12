# M39 实施记录

基线：`e88f7d73`（仓库历史压缩后的实际 HEAD），2026-10-12 开始。工作区初始干净，无 target 目录。

## 顺序与当前状态

1. 已同步三份规范与路线图：明确布局／签名准备、codegen ABI 规划、完整 LIR 完成，typed 身份与消费者支持边界，工具执行及 runtime 组件职责。
2. 进行中：C toolchain 宿主 invocation 移出 LIR，generated-C 编译交由 driver 编排。
3. 待实施：目标身份／目录／LLVM 配置拆分，全部 Scoop/C ABI 规划迁移。
4. 待实施：对象格式／机器规则、工具链与 runtime 组件选择。
5. 待验收：本机定向回归、Linux Rust 与正式 CLI 完整覆盖、边界审查及清理。

## 验证环境与纪律

- 本机使用 LLVM 22.1：`/opt/homebrew/opt/llvm@22`；临时文件与日志放在 `target/m39/`，设置 TMPDIR 指向该目录下的 tmp。
- Linux 环境为 `nuc12:~/repos/scoop`，现有远端工作区包含上次 M40 同步留下的未提交修改，后续同步前逐文件核对，保留非本任务内容；不重置远端工作区。
- 每批 Rust 变更先 fmt、workspace clippy，再定向测试；正式 CLI 复用已有真实功能组合，避免重复全量测试。
- 当前仅记录历史验收为参考：M40 三 target 完整 fixture 已通过；本次尚未执行的测试不计为通过。

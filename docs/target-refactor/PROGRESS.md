# Target 重构与 macOS x86_64 实施记录

状态：实施中。范围与边界见[设计](DESIGN.md)。

- 分支：`codex/target-refactor-macos-x86-64`。
- 起点：`09be3315a85cfa9e50f79e49785bd23e0880f1f0`，包含 Windows targets 与 AppKit／Win32 窗口库合入。
- 本阶段完成目标边界重构并加入 `x86_64-apple-darwin`；Linux ARM64 GNU／musl、OpenHarmony AMD64／ARM64 留待后续。
- 每批完成格式化／lint 和相应验证后提交、推送，再获取并合并远端 `main` 的新增提交。原始日志放在仓库外。

## 环境

本机为 Apple Silicon，macOS 27.0.1，Xcode macOS SDK 27.0。`arch -x86_64 /usr/bin/uname -m` 已成功；本阶段新增目标通过本机 Rosetta 执行，结果不视作真实 Intel 硬件验收。部署版本和 SDK slice 在实际编译时确认，尚未承诺旧系统运行范围。

## 批次

| 批次 | 状态 | 范围与结果 |
| --- | --- | --- |
| R0 | 完成 | 完成 Windows 后设计复审，更新本阶段范围及目录所有权规范，创建实施分支 |
| R1 | 待实施 | 静态目标目录、宿主选择与公共脚本元数据 |
| R2 | 待实施 | 显式 ABI／backend／EH／frame 与 codegen 调优数据 |
| R3 | 待实施 | 对象 machine／格式与 runtime frame 边界 |
| R4 | 待实施 | 工具角色、架构参数、SDK 与 Linux Clang |
| R5 | 待实施 | native／RC 的 target sysroot include 与维护流程 |
| R6 | 待实施 | macOS x86_64 实现、资源与本机运行 |
| R7 | 待实施 | 七目标回归与完成记录 |

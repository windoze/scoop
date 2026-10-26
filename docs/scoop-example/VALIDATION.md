# scoop example 本机验证

日期：2026-10-27。宿主：Darwin/AArch64。范围为独立外部命令、首批 24 个主题及安装打包；不修改编译器源码，不执行编译器测试套件，不做 Linux 验证。

## 前置工具链更新

开始实现前，已通过完整打包入口更新四个宿主命令、系统库、native/runtime 资源和离线 API 文档，并成功生成基线安装包。准备时发现 runtime_builder 的独立 lockfile 缺少 scoop-identity 已使用的 rustc-hash 2.1.3，已补齐锁定记录；未修改编译器实现。

## 独立命令与内容

- 独立 Cargo 项目的 fmt、clippy（警告视为错误）通过；9 项测试覆盖索引约束、目录/别名/检索、三种格式、每个文件的原文一致性、文件选择、JSON 错误与退出码、缺失输入不产生半份正文、从自身位置选择安装根。
- 本次 Python 文件经过 ruff 0.16.10 格式化与检查；5 项独立打包测试通过，包含命令权限、归档布局、示例数据文件，以及两个 Cargo 项目共享依赖的许可证收集。只收集正常/构建依赖，不带入测试专用依赖。
- 新增 Scoop 程序通过现有 scoop fmt 格式化；独立 run_examples.py 从实际 scoop example 的 JSON 输出还原文件，24/24 主题通过本机 release 编译和运行。包括完整两 Cone 调用、临时文件清理、异步取消收尾、本机 HTTP client/server、CLI 帮助与错误码、2 个断言测试及 list/filter。
- 已通过真实 scoop 外部入口检查显式 --sysroot 的选择；阅读器输出所选安装包的 VERSION，不重新采用 cwd 或 SCOOP_SYSROOT。

首轮发现并修正了 JSON 默认省略 None 字段的说明与断言。HTTP 测试启动检测从重定向 stdout 改为实际监听端口，避免输出缓冲导致误判；完整复验已通过。原始源码、工具与产物始终保留。

## 发行验收

完整打包入口成功生成 `scoop-0.0.0-dev-aarch64-apple-darwin.tar.xz`（419,640,684 字节）及配套 LLVM 源码包。包内源码 revision 为 `e407dcbf279177d0047c4c008b5f5f91e8ef8f4c`。

解压到新的安装目录后，通过包内 scoop 查询全部 24 个主题，32 处源码引用与包内文件原文一致；33 个阅读器生产/构建依赖的许可证目录齐全。显式 --sysroot 在环境变量指向另一份已准备安装根时仍选用正确内容；默认安装根入口与 source 输出也正常。使用解包后的工具链独立运行 start/hello 和 json/typed，2/2 通过。

基线包在 `scoop-work/scoop-example/baseline-dist`，最终包在 `scoop-work/scoop-example/dist`，均为仓库外的相邻工作目录。构建日志、示例还原目录与逐项日志也保留于该工作目录，不纳入版本控制。

后续规划的 66 个主题没有注册到发布索引，不属于本批完成范围。HTTP 本机验证使用 127.0.0.1:8080；只终止测试自身启动的服务进程。

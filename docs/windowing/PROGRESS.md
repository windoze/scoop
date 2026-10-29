# 窗口基础架构与 macOS 实施

本阶段：基础架构、`scoop.window`、`scoop.window.asyncio` 与真实 AppKit 后端。Linux／Windows／其他后端后续实施，不添加空实现。窗口逻辑、平台适配与测试均归属普通 Cone；编译器只补齐实际需要的 `.m` native 能力及实施中暴露的底层缺口。

实施顺序：

1. Darwin `.m`／ARC／独立 `objc_flags`，验证正常 framework 与 Objective-C 链接。
2. 同步窗口规范，迁入统一的 asyncio notification／watch 接口。
3. 公共值类型、Application／Window 生命周期、proxy、native lease 与真实 AppKit 主循环。
4. 窗口属性、输入／文本批次、重绘、显示器及 macOS 平台能力。
5. 窗口宿主与 asyncio 接入、取消和退出收尾。
6. Cone 测试、真实 surface 示例、回归验收与文档。

每批完成后先格式化／lint，再运行相应测试，提交并推送分支，检查并合并远端 main。原始日志与临时验收产物保存在仓库外；本文只记录范围、结果与实际限制。

## 已完成

### 1. Darwin Objective-C native 管线

- `.m` 使用所选 Apple Clang、ARC 与独立 `objc_flags`；条件筛选先于目标限制，禁止以 flags 覆盖 ARC／输入语言，`.mm` 明确拒绝。参数、头文件与对象复用沿现有 native 构建记录处理。
- PM／scoopc 展开配置同步到 package protocol 3／compiler protocol 23；未增加语言语法、runtime ABI 或窗口专用产物。
- 实际 AppKit 链接暴露并修复两项底层缺口：多个 framework 重导出同一最终定义不再误报歧义；Apple Clang 的 selector stub 交给 Darwin linker 合成，并正常解析真正的 `objc_msgSend` 运行库依赖。
- 同步窗口规范中已过时的文本编辑批次、native lease 与跨 Cone 宿主接口；Linux／其他后端不属于本阶段。

验证：workspace fmt／clippy 通过；manifest／protocol／toolchain 共 176 个 Rust 测试通过，linker 测试通过；fixture runner 公共规则测试通过。Objective-C 3 个 fixture、4 个变体、12 次真实进程调用通过，覆盖 debug／release、ARC 弱引用、AppKit／Foundation、混合 C、直接复用、flags 变化及错误位置。现有 native C++ 10 个适用 fixture（13 个变体、40 次进程调用）通过；其余 3 个目标不适用项未计为通过。开发 sysroot 的 25 个 Cone 与 runtime 已显式准备。

### 2. 普通窗口 Cone 的值基础

- 新增 `scoop.window`，提供独立实体 id、窗口错误／状态／能力类型、逻辑与物理坐标、完整 metrics，以及向上取整与溢出检查。
- 文本输入提供带方向的 UTF-8 选区、快照、有序编辑批次及纯值应用；整批校验、一次 revision 更新，覆盖临时文本范围、组合替换与 surrounding 删除。
- sysroot 维护构建与文档共用目标源码清单；Darwin 专用 Cone 不进入 Linux 清单，依赖不可用仍正常报错，不增加编译器配置或平台空实现。

验证：Scoop 格式化／check 通过，所属 Cone 的 9 个 release 测试通过；Ruff 0.16.10 格式化／lint 通过，脚本的 12 个测试通过。AppKit 后端与应用对象尚在下一批实施。

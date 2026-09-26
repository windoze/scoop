# core 普通 library 与 M23 清理

共有声明表允许保存实际编译使用的 internal/private 顶层支持声明，包括初始化服务；可见性仍控制公开查找。reader 只核对声明关系中的 constructor、member、child、enum variant 和 accessor 引用完整，不另以从 public roots 可达为来源资格，也不为此再次遍历签名、binder 与默认值 body。对应类型、参数/default 和访问关系由各自消费边界检查并复用结果。

初始化循环异常服务是前端已解析的实际 typed 函数声明。声明以原可见性进入共有 callable 支持记录，实际 MIR body、LIR canonical ABI、导出与依赖选择均使用普通 callable 表；internal 服务不加入 public lookup。`InitializationCycle` 只表达 lowering 选择失败分支目标的语义角色，不产生来源资格、第二份 ABI 或独立 Link owner/requirement。lowering 生成的调用可以没有源码 lookup 记录；已有源码调用仍核对实际目标、provider、参数、结果与物化根，所有生成调用仍核对完整 typed 依赖和 ABI。

Strong production 的两种表示使用 `/11`、`/12`，删除初始化专用 ABI field 11 和外部服务表 field 12，section 只保留 field 2～9 的八字段 product。旧 field 1、10、11、12 及服务表 tag 1、2、3 全部退役且不得复用；旧产物和缓存按版本规则重建。普通 MIR/LIR callable、layout/ABI、registration 与实际 provider/typed target 继续承担完整调用和物理引用信息，不保留空表或兼容双轨。runtime C 调用约定、String 表示和登记语义不变。 `link-identity-closure` 同步升级为 `/3`，退役旧 final undefined requirement 的服务专用 tag 8 及 object-definition fingerprint 的服务专用 tag 13，均不复用；普通外来 callable 继续使用现有 `cross-cone-link-closure/1` 的 typed target 记录，runtime 编码不新增服务分支。 仅由旧测试使用的 single-Cone 发布凭证、独立 Compile/Link 双重读取与第二套 atomic publisher 一并删除；正式 M23-6 发布继续使用完整编译输出及共有原子写入路径，普通摘要保留。

本文件记录当前实施边界，不将历史测试数量、冻结条款或旧证明链作为完成条件。规范以 [语言规范 12.6](../specs/SCOOP-SPEC.md#126-核心库)、[实现规范 2.12](../specs/SCOOP-IMPL-SPEC.md#212-core-普通库的共有验证) 和 [运行时规范](../specs/SCOOP-RUNTIME-SPEC.md) 为准；实际功能见 [M23-6 设计](stage6/DESIGN.md)。

core 是可由用户修改、扩展和重建的普通 library Cone。sysroot 只提供默认查找位置，显式依赖优先；构建、缓存、类型查询和产物消费都使用共有路径。前端负责 intrinsic 识别和语言声明规则，后续 stage 消费完整 typed IR 与实际声明引用。保留 Unit/Any 的语言内建身份，不按 FQN、同名或相同布局恢复其他实体。

## 清理范围

[清理设计](stage6/CORE-AUTHORITY-CLEANUP.md) 的七项均属于本阶段：

1. canonical ABI 不按 CORE 身份跳过、豁免或要求专用空表。
2. native boundary 从共有依赖类型查询取得真实声明、表示及 FFI 合同。
3. compiler protocol 保留语言角色的 typed 引用，删除来源资格、重复投影及凭证。
4. String、初始化与 Link 使用实际 provider、typed target 及共有 descriptor/callable/selected/registration/definition 路径。
5. 删除 compiler/slib/runtime 的通用预算、计费、配额及其版本和 ABI 绑定。
6. 删除正常路径中对相同不可变事实的多轮完整语义、布局和 scan 验证；Compile/Link、发布和 runtime 热点复用已完成的结果。
7. 删除测试专用来源工厂、重复语义实现、凭证状态机和只为废弃机制存在的数据、适配层及测试。

范围覆盖普通 Cone 与 core。把专用授权改成共有授权，或保留 unlimited/空壳计量接口，均不满足目标。必要的类型、可见性、格式、引用、依赖环、缓存失效、符号、ABI、对象范围与 GC 检查继续保留。默认参数的定义检查由前端负责，完整 typed 正文及实际目标供跨 Cone 实例化使用。

## 实施状态与验收

当前 M23-6 尚未完成。已有 canonical ABI、native boundary、protocol 和 Link 的共有化改动，需按生产调用链确认后保留；不因旧名称仍存在就重新实现或机械删除。历史 source-authority、预算和重复重放测试的通过记录不代表实际功能验收。

每项完成须记录对应功能提交与实际验证：

- 从真实 provider/core 源码生成完整产物，consumer 只读产物且 provider 源码不可见。
- core 在普通目录修改、增加公开声明、重建后，下游读取新内容；缓存按内容变化失效，损坏显式输入不能回退 sysroot。
- 类型布局、成员、constructor、dispatch、protected、ABI、ZST、boxing、初始化和 String 的独立与组合场景符合阶段成功矩阵。
- Compile/Link 拥有完整数据，实际对象、符号、relocation 与 registration 一致；完成本阶段适用的链接运行。
- 旧格式按版本拒绝并要求重建，退役 tag 不复用；fixtures/golden 与文档同步。语言错误、格式错误和正常功能回归保留。
- 每批代码先格式化和 lint，再运行影响范围测试，验证通过后按功能提交。检查无构建或测试占用后定期清理 target。

只有上述实际功能和七项清理均完成，生产路径不依赖已废弃机制，必要验收通过，文档与实现一致，且变更按功能提交，才标记目标完成。ODR、multi-image startup 和 artifact-only program-link 仍由后续里程碑负责。

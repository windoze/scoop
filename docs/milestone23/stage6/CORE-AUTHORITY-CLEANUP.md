# M23-6：普通编译器职责与过度设计清理

本设计落实最新版 AGENTS.md，并明确修正旧目标和设计中保留通用预算、来源证明、任意成本模型及重复完整验证的条款。清理覆盖 compiler、slib、runtime 和所有相关规范、格式、测试，不限于 core；将 core 专用机制改成所有 Cone 共用的机制不等于删除过度设计。

配套：[语言规范 12.6](../../specs/SCOOP-SPEC.md#126-核心库)、[实现规范 2.12](../../specs/SCOOP-IMPL-SPEC.md#212-core-普通库的共有验证)、[运行时规范](../../specs/SCOOP-RUNTIME-SPEC.md)、[M23 总设计](../DESIGN.md) 和 [M23-6 设计](DESIGN.md)。

## 1. 边界与完成条件

保留并完成 M23-6 的类型布局、canonical ABI、dispatch、ZST、跨 Cone 实际消费和产物发布。core 是普通 library Cone，用户可修改、扩展和重建；sysroot 只是默认查找位置。前端负责 intrinsic 识别和语言规则，后续阶段只消费完整 typed IR、实际声明及 provider。

保留全局唯一且类型化的实体 ID、类型与可见性、格式/引用、依赖环、内容 fingerprint、缓存失效、符号/ABI、对象范围和 GC 契约。普通来源位置、依赖路由和声明引用有实际语义用途，不按名称机械删除；它们不承担授权、防伪、反篡改或资格证明职责。

## 2. 七组清理

### 2.1 canonical ABI

删除 `ConeIdentity::CORE` 的验证跳过、豁免和专用空表限制。从实际签名、exact type、target、布局及 provider 使用共有规则，空 expectation 自然成功。普通 callable bridge 与 layout profile 均核对真实 ABI，但同一不可变签名/布局不因消除豁免而多轮完整重算。

### 2.2 native boundary

删除 `TrustedCore`、`ImportedCoreNativeBoundaryTypes`、`CoreNativeBoundaryNominal` 及 external-core 专用恢复路径。共有依赖类型查询提供真实 nominal、字段/variant、参数、CLayout policy 和表示。后端消费正规化 typed representation，保留 FFI 的 C-safe、GC-free、布局、调用约定和根契约；Unit/Any 维持其语言内建身份。

### 2.3 compiler protocol

删除 CORE 来源资格、Core/NotCore 包装、重复 operation/签名投影和凭证外层。语言角色只保存实际需要的 typed 声明引用、签名、effect、可见性与依赖关系。普通 metadata 发布和消费这些声明；intrinsic 正规化后不再从来源获得执行资格。internal 服务不因迁移加入 public lookup。

### 2.4 String、初始化与 Link

String、初始化服务及 descriptor、callable、selected、registration 接入共有记录。删除固定 CORE provider、专用 bridge、独立授权及 core requirement/owner/proof 通道。所有外来 strong 引用按实际 provider 和 typed target 查询 definition、ABI、symbol 与 relocation；GC 登记与初始化依赖仍完整。runtime/native/target 的不同调用契约保持，用途分区互斥且完整覆盖实际 undefined relocation。

### 2.5 通用资源预算与计费

删除 `BudgetMeter`、累计 usage、logical heap/work、节点/边/复制字节和逐操作收费的参数、接口、错误、版本字段及专用测试。不保留改名后的 meter、unlimited 或空壳。

runtime scan 比较删除任意展开次数、逻辑字节配额及超额 abort；成本常量不再进入 profile、fingerprint 或 runtime ABI。实际 offset/count/length 使用 checked 运算并核对真实存储范围，非法循环由局部图检查处理，共享 DAG 比较按节点或节点对复用结果。分配失败正常报告，不以成本估算决定合法程序能否编译或运行。

### 2.6 重复证明与完整验证

同次编译使用完整 HIR/MIR/LIR 直接生产导出和对象，不反复逆向制造同一对象以证明来源或操作资格。外部产物在读取边界完成格式、typed 引用与跨层关系检查；Compile/Link 对同一字节快照和依赖复用该结果。Link 追加真实对象范围、符号、relocation、registration、patch 和 Code/runtime fingerprint 检查，不能删除必要检查或接受损坏产物。

发布复用同次编译结果及已检查依赖，不对当前产物和全依赖分别重开 Compile/Link 并完整重演。临时文件写入、同步与回读只用于确认实际写入内容，成功才原子替换。

M23-6 的 runtime 复用编译器或 artifact reader 已验证的静态类型、布局与 scan，分配、装箱、数组及 GC 不反复完整校验。动态对象范围、length/size、TD 归属、对齐和 GC 根仍检查；装箱根直接引用 TD 的 inline scan，不重发射和逐项比较第二份 scan。完整静态检查保留为 `scoop_shape_validate`，`SCOOP_VERIFY_METADATA=1` 可在 runtime 操作入口显式启用。M23-8 才引入多 image 登记边界，本阶段不为此新建 registry 或不可变指针缓存。

### 2.7 无生产用途的框架和重复实现

删除仅由测试实现/使用的 HIR/MIR/LIR 来源工厂、平行来源 reader、凭证状态机、重复数据表及适配层。旧 bootstrap stage 包装与 Core/NotCore 测试输入适配层同样删除；保留的真实源码测试直接调用共有 stage 入口。独立 dual-artifact certificate/reopen 框架没有生产调用者，删除后只保留实际使用的不可变产物快照。source-authority 收缩为名称解析、类型检查、默认参数实例化和跨 Cone 消费实际需要的声明/来源信息，不保留逐项授权和防伪证明。

语言规则由对应 stage 负责；IR/meta crate 负责数据及格式/引用不变量，reader 不再维护一套前端语义实现。已解析默认值保留完整 typed 正文、声明引用和定义环境。不得用新通用工厂、插件、安全框架或证明系统代替删除项。

## 3. 编排、wire 与文档

producer、reader、linker、wire/profile、版本、fingerprint、fixture、golden 及规范同步修改。格式改变按正常版本演进，退役 tag 不复用；旧产物明确重建，不做长期双轨适配。内容 fingerprint 与缓存记录继续保留实际用途，不绑定计费策略或来源授权。

保持实际 runtime C 调用约定、String 表示与 persistent identity。后续 ODR、multi-image startup、artifact-only program-link 不属于此清理。历史阶段文档如描述已退役策略，只可作为带有明确迁移说明的历史记录，不能继续作为生产入口要求。

## 4. 实施顺序与验收

先修订三份 spec、ROADMAP、M23 总设计、M23-6 设计与本清理文档，再按实际调用链清理实现。已完成项核对后保留，不重复实现；每批代码先格式化和 lint，随后测试，通过后按功能提交。

| 验收项 | 实际场景 |
|---|---|
| core 普通库 | 普通目录修改、扩展、重建，显式依赖优先，下游消费与缓存失效 |
| 类型与 ABI | 独立/混合 provider 的 layout、C-safe、完整签名、ZST、direct/indirect、GC/root plan |
| 成员与 dispatch | constructor、继承、interface/default、getter/setter、protected、真实 body/slot |
| String 与初始化 | 字面量、类型测试、属性/object 初始化、循环失败、真实 descriptor/callable/registration |
| 产物与 Link | 真实源码生成完整产物，consumer 只读产物；对象、符号、relocation、版本与内容检查 |
| runtime | 分配、boxing、array、GC 不重复展开已验证静态 scan；动态范围错误仍拒绝 |
| 清理结果 | 无生产调用依赖旧授权、通用计量、重复证明或测试专用工厂；不以改名或 unlimited 通过 |

语言错误 fixture 断言位置与信息，格式损坏和正常功能回归保留，删除仅服务旧机制的测试。手工 metadata、测试来源工厂和大量证明反例不能替代真实编译、跨 Cone 消费、适用链接运行。只有主设计第 15 节的原有功能、本文件七组清理及验收均完成、文档一致并按功能提交后，才能完成目标。

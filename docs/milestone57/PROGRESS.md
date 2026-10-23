# M57 实施记录

状态：实施中。更新：2026-10-23。目标为 [DESIGN.md](DESIGN.md) 的普通类型与 companion 条件 interface 实现；整份类型特化不在本期。

## 1. 基线与工作方式

- 基线：`f3f8ace69`；分支：`codex/m57-conditional-interfaces`。
- 已同步语言 9.1.8、companion 投影与 codec 来源合同，以及实现／运行时规范的条件关系和接口表职责。
- 分功能完成并提交。本机先做相应格式化、lint、单元／Cone／必要 CLI 验证，再在 `nuc12:~/repos/scoop` 做 Linux 完整验收。
- 临时文件使用仓库外 `../scoop-work/m57-temp`，原始日志使用 `../scoop-work/m57-validation`，不使用 `/tmp`。
- 开始实施前用 Cargo 清理旧 debug 产物，target 从约 28 GB 降至约 7.2 GB；后续在无活动构建时按实际体积继续清理。

## 2. 功能状态

| 功能 | 状态 | 验证证据 |
| --- | --- | --- |
| 语法与普通类型条件接口 | 本 Cone 基础闭环已完成；继承等组合另行验收 | 4 个 parser 用例、11 个 M57 HIR 用例、8 个诊断 fixture、4 个 core consumer 原生用例通过 |
| Companion、HashMap 与 Json 闭环 | 已完成 | 6 个 JSON Cone 原生用例覆盖四种能力组合、嵌套、显式工厂及重复 key 路径 |
| 条件 codec 合成 | 已完成 | 2 个定向检查、24 个既有合成回归、完整 JSON Cone 76 项测试及跨 Cone 合成 codec 运行通过 |
| 继承、default、property 与效果组合 | 继承与 default 基本闭环已完成；其余组合待完成 | 条件继承、等价重列、域覆盖、abstract 义务、默认方法／属性及分派有本地／跨 Cone 验证 |
| 导出、跨 Cone 与 ODR | 已覆盖继承、投影与 default；完整验收待完成 | 85 个泛型导入回归、条件记录 wire 往返／再次发布与双 consumer 原生链接运行通过 |
| 完整组合与 Linux 验收 | 待完成 | 尚无完整验收 |

此表只在实际代码及对应验证完成后更新，不以规范已写入代替功能交付。

## 3. 基础闭环

- AST 保留独立实现块；普通类型和 companion 复用原宿主参数，方法／计算属性的条件不改变类型布局或单例身份。
- 支持条件关系、直接调用、泛型转发、方法引用、属性访问、接口转换和动态接口判断；普通 override 可满足多个条件接口义务。未证明的条件不成立为接口事实，也不能作为 M55 负向约束的证据。
- 实现重叠、重复签名、无关 override、非法存储和负向条件有正式 CLI 诊断覆盖。循环／扩张查询被拒绝，有限嵌套递减和独立成功证明有 HIR 覆盖。
- Concrete 接口关系从声明求值，不读取尚未构造完成的接口表；递归基类实例化顺序有原生覆盖。
- 本机 workspace Clippy 通过。HIR 回归首次 1,526 通过、15 失败；失败均为导入 intrinsic 的独立存储路径，修正后相关 128 个回归用例通过。后续关系求值调整通过 M55／M57 共 29 个用例。
- 已显式准备 Darwin 的 21 个系统 Cone 与 runtime；正式验证消费共享 sysroot／Cone cache。跨 Cone 条件元数据、复杂继承／default、codec 合成及 Linux 验收仍待后续步骤。

## 4. 条件关系的跨 Cone 消费

- `cross-cone-interface/79` 保存原宿主 binder 下的条件声明、成员适用性及带完整 owner application 的 slot 选择；导入后沿用定义处目标，互斥条件下的 default 选择保持独立。
- 参数的 public companion 声明在具体化前随实际泛型实参保留，避免仅使用 Any／动态判断时漏掉接口关系；查询不执行 companion 初始化。
- 无泛型的实现块同时导出普通继承边和接口槽位，class／struct／enum／object 均有从源码到 MIR 分派表的验证。
- 本机 workspace Clippy、867 个 HIR 格式测试、83 个泛型导入回归通过；16 个 M57 定向用例覆盖本地、导入及产物分派。格式版本变更导致的一项 slib 期望向量失败已更新，相关 17 个 profile 测试通过。
- 正式 ODR fixture 由 provider、两个 library consumer 和 root 构成，验证条件真／假、非泛型实现块、单例共享及不同实参独立；原链接与逆序依赖重链接均运行成功。Darwin sysroot 已按新格式显式重建。
- HashMap 默认 codec、继承／合成组合、完整 Linux 验收仍未完成。

## 5. HashMap 的默认 codec

- HashMap 原 companion 分别声明 ValueEncoder 与 ValueDecoder 条件实现，使用 K／V 的 public companion；保留原存储、键约束、显式工厂及 map helper。
- JSON Cone 的 6 项原生测试通过：编码／解码独立，两个组件均须满足对应条件；没有 companion 仍能正常使用容器及显式 codec；覆盖嵌套 HashMap、泛型转发、原 entry 表示、新建解码结果和重复 key 的 `/1/0` 路径。
- 明确验证显式 codec 优先于已有默认能力。其他容器未自动迁移，条件 codec 合成及继承组合继续实施。

## 6. 条件 codec 合成

- 空实现块的既有 codec 合成保留原条件及成员适用性，合成正文使用完整 B+C 环境；字段 codec 可以使用已证明的参数 companion 投影。
- 保持显式 primary val 依赖、递归 this、普通 companion、原容器／tuple 组合的优先级。显式方法阻止同签名合成；不具备正确字段 codec 的条件在声明处报错，诊断定位到实现块。
- 2 个 HIR 定向检查与 24 个既有 encoding／decoding 合成回归通过；正式负向 fixture 验证不能用 decoder 条件替代 encoder 条件，反向亦然。
- 完整 JSON Cone 的 76 项原生测试通过。新增 5 项覆盖独立方向、Envelope 与 HashMap 等完整泛型字段、显式依赖与方法优先、递归节点和默认值。首次失败为新用例误把应省略的默认 None 字段写成 null，按现行 wire 规则修正预期后通过，未更改编码规则。
- 跨 Cone ODR fixture 同时消费并执行 provider 合成的 Envelope codec，原链接及逆序依赖重链接均通过。

## 7. 继承条件与分派

- 基类的条件 conformance 沿完整 base application 代入，保留原 owner 与条件身份；重列接口要求在派生类基础约束下等价。普通 override 可覆盖条件 slot，带条件的 override 必须覆盖继承义务的完整域，不能用自身条件证明覆盖。
- 具体子类检查全部可能适用的 abstract 义务，已证明不可能的条件不产生义务。构造类型实参的条件使用代入后的声明事实，不反向猜测其内部参数约束。
- `cross-cone-interface/80` 增加检查后 conformance 记录，导入方直接消费原关系；空接口与继承重列也保留完整条件。普通源声明的虚表描述复用具体 HIR 实际保留的 family；无条件 override 在基类方法不可用时仍保留自己的合法成员与分派，不要求生成不适用的基类正文。
- 本机 workspace Clippy 通过。25 项 M57 定向检查、867 项 HIR 格式测试、84 项泛型导入回归、97 项普通规则回归、121 项类型／产物回归通过。格式版本对应的固定向量已更新，17 项 profile 检查通过。
- 已显式按格式 80 准备 Darwin 的 21 个系统 Cone。完整 core consumer 的 41 项原生测试通过，新增 3 项覆盖普通／条件 override、条件关闭后的虚表位置、构造实参、abstract 与 computed property。跨 Cone ODR fixture 增加相同继承组合及 marker 接口，原链接／逆序依赖重链接均通过；10 个条件接口诊断 fixture 通过。
- 继承 codec 合成、companion 出现在条件接口目标／成员签名、default 条件交叉、sealed／类型描述／文档及 Linux 完整验收仍须继续完成；本节不代表这些组合已验收。

## 8. 条件目标与成员签名中的 companion 投影

- 已确定不成立的继承条件在目标／成员签名替换前排除；仍未知的条件继续保留。判定约束之前先检查全部必需投影的存在性，支持一项参数约束引用另一参数的 companion。
- 成员签名比较不展开已知不适用的投影，调用候选仍保留原有约束失败诊断。跨 Cone 的条件槽位同样先判定适用域，避免为没有 companion 的实参解析投影接口。
- workspace Clippy 与变更后的定向 Clippy 通过，29 项 M57 检查、85 项泛型导入回归、10 个诊断 fixture 通过。完整 core consumer 的 42 项原生测试及 ODR fixture 的原链接／逆序依赖重链接通过；新增用例覆盖接口参数、方法参数／结果、继承、条件关闭及子类普通成员。
- 后续继续处理继承 codec 合成、可同时成立的 default 条件、sealed／类型描述／文档以及 Linux 完整验收。

## 9. 条件 default 的冲突与实际选择

- 定义处检查可能同时成立的接口域；无普通子接口覆写关系的默认实现要求覆盖两边条件的共同 override，不依赖当前是否已有实例。kind／不相关 class 上界可证明的互斥域保持独立。
- 默认成员查找先证明所属条件，再按普通子接口关系消歧。具体接口表按 slot 合并实际适用的选择，源码块顺序不决定实现；直接调用、接口调用和泛型转发使用同一份结果。
- 已先同步实现规范，普通外部成员的类型依赖投影复用 HIR 的成员绑定，不从无条件声明头重复证明接收者关系；保留调用签名、类型引用和声明归属检查。未在 IR／meta 中新增条件求解器，产物格式仍为 80。
- workspace Clippy、33 项 M57 检查、85 项泛型导入回归、97 项普通规则回归及 866 项 HIR 元数据测试通过。完整 core consumer 的 44 项原生测试通过，新增用例覆盖默认方法／属性、块顺序、无条件父接口、共同 override 与未知条件下的泛型转发；ODR fixture 的原链接／逆序依赖重链接通过。
- 11 个条件接口诊断 fixture 通过，覆盖未实例化时的潜在 default 冲突、继承冲突、未覆盖全部条件的成员以及条件未知／不成立时的 default 调用；已有重复成员与 override 诊断保持独立。
- 继承 codec 合成、sealed／类型描述／文档、其余效果组合及 Linux 完整验收继续保留为未完成项。

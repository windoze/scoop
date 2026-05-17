执行计划

1. 读取 TODO.md，按标题是否带有 [DONE] 判断第一个未完成任务，并同时检查最新提交信息是否明确提到与该任务直接相关的未完成事项。
2. 读取该任务涉及的上下文文件，只做完成当前任务所需的最小范围调查，不进行开放式历史问题扫描。
3. 如果发现当前任务被真实前置缺陷或缺失功能阻塞，则把最小必要前置任务插入 TODO.md，记录依赖关系，提交后停止。
4. 如果未被阻塞，则按任务要求实现代码、夹具或文档变更，避免绕过规格或缩窄测试形状。
5. 运行与该任务相关的测试和必要的验证命令；若失败，修复同一根因影响的完整问题类别后重新验证。
6. 更新 TODO.md：在任务标题前加 [DONE]，补充完成记录和验证结果。仅当阶段级计划变化时才更新 PLAN.md。
7. 提交本次任务相关的全部未提交变更，提交信息使用任务编号和简明描述。
8. 停止，不继续处理下一个任务。

进度记录

- 已创建初始执行计划，下一步读取 TODO.md 识别第一个未完成任务。
- 已读取 TODO.md；第一个未完成任务是 P13-T01：更新 spec §10.3，删除 `var StringBuilder.lastChar` 示例，加入 `scoop.lang` 简介和 sysroot 目录组织约定。最新提交 `[P12-T05] Narrow sysroot origin semantics` 未声明与该任务直接相关的未完成事项。
- 已读取 TODO-5.md 中 P13-T01 详情；任务限定为 spec 文档修改，不改代码。索引标题包含 sysroot 目录组织约定，后续会在 `SCOOP_FULL_SPEC.md` 中与 `scoop.lang` 简介一并补充。
- 已完成 `SCOOP_FULL_SPEC.md` 初稿修改：删除 `StringBuilder.lastChar` 示例，新增标准 cone / `scoop.lang.string.StringBuilder` / sysroot 目录约定说明，并更新 intrinsic/sysroot 交叉引用。
- 已完成验证并回写任务状态：`TODO.md` 索引与 `TODO-5.md` 的 P13-T01 标题均已标记 `[DONE]`，完成记录已写入验证结果。下一步检查工作树并提交本任务变更。
- 已补充 `git diff --check` 与 `cargo clippy --all-targets -- -D warnings` 验证并同步更新完成记录。提交时仅纳入本任务相关文件，保留现有未跟踪 Markdown 文件不动。

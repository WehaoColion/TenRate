# Android 2.23.4 候选版：Rust 目录新建轻量化与保存延迟分段诊断

## 背景

用户在“知识目录 / agent 消耗”下新建标题“001”的页面，界面长时间显示“正在保存…”，约持续五六分钟。仅凭截图无法确定本次究竟在等待队列、锁、原生快照验证、完整快照生成，还是回调。历史设备日志曾出现分钟级 `snapshot_preflight`，但这不是本次操作的直接分段证据。

目录入口与 2.23.2.59 的空白编辑会话入口不同：`knowledgeContainerAction` 仍执行完整 AppData 的 Kotlin JSON 编码、JNI/Rust 解析与规范化、Kotlin 解码以及 `updateDataDetailed(persistImmediately = true, publishAfterDurable = true)`。本轮只在可以确保语义等价的地方减少 Rust 目录层级计算，不改动安全存储架构。

## 变更

1. **Rust 新建目录节点的兼容文件夹投影局部化。** `knowledge_container_action_app_data_json` 在新建页或新建文件夹且没有已有节点引用该新 ID 作为父级时，仅沿新节点父级链求最近的旧版文件夹 ID，并沿用原有时间戳修订规则；不为该节点重新构造完整的父子索引及后代投影图。原有 `parse_app_data_for_mutation` 与最终 `data.sanitized(now)`、整体 JSON、保留数据及权限校验不变。
2. **对特殊数据安全回退。** 若已有文档（包括删除状态记录）引用新建节点 ID 为父级，可能构成历史孤立子项的重新挂接，此时继续使用原有完整投影。移动、重命名、页面文件夹转换和旧文件夹升级均继续使用完整投影与现有保护逻辑。
3. **分段诊断目录操作。** 在 `TimerRepository.knowledgeContainerAction` 记录超过 1000ms 的 `hierarchy_encode`、`hierarchy_rust_mutation`、`hierarchy_decode`、`hierarchy_action_total`；现有保存队列的 `explicit.queue_wait`、`explicit.operation` 与 `snapshot_preflight`、`preflight_native_verification`、`sqlite_commit`、`recovery_mirror` 可共同定位。日志只含固定阶段名及耗时，不包含标题、正文、页面 ID、账号 ID、工作区键或凭据。未追加轮询、超时取消或伪成功回执。
4. 安卓候选版为 `2.23.4` / `22330`，沿用原包名、正式签名证书和快照/同步格式。候选分支新增 Rust 目录测试到既有签名 APK CI。

## 测试与界限

- Rust 等价性回归：根目录、普通多级父级、旧版文件夹和其多级后代，分别新建页面与文件夹，对比优化路径和旧版完整投影后的整个 AppData JSON。
- 孤立子页回归：历史节点引用未来才创建的父级 ID，父级创建后继续使用全量投影并更新其旧版文件夹位置，不删除或误迁移原子项。
- 源码生成回归：诊断阶段存在、仍采用非取消事务、`persistImmediately` 及 `publishAfterDurable`，不在日志输出页面标题或工作区标识。
- 设备验收：原样保留用户数据，在创建小页、旧版目录挂接、复杂知识库、异常退出后重新启动等情况下，分别核对耗时、页面完整性与版本恢复。CI 通过不等同真机性能达标。

**严格限制：** 本轮没有移除完整 AppData 的来回序列化，也没有减少保存前历史快照的认证量。因此即使 Rust 层级计算变快，也不能承诺修复分钟级保存等待。下一步是否做增量存储与可靠校验复用，应依据这次新增日志定位实际最重阶段后单独设计、审查和迁移。

本 PR 为独立候选，不自动合并，不覆盖用户设备数据。

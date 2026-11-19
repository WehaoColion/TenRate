# Android 3.0.2.1：知识目录单节点增量创建与持久化验收

- **版本**：3.0.2.1（versionCode 30003）
- **范围**：Android 知识模块中的“新建子页面 / 新建子文件夹”；Windows、同步协议、风控及持久化格式不变
- **状态**：PR 候选，待 Rust 测试、签名构建与实际 Android 模拟器确认；不自动合并

## 背景与定位

2026 年 11 月的知识目录截图表明，创建仅有名称的子页面也会在禁用“确定”和“取消”的弹窗里长时间显示“正在保存”。代码路径包含完整 AppData 的 Kotlin JSON 编码、Rust 原生解码与数据修改、整份 AppData 的 Rust 编码、Kotlin 反序列化，随后再执行严格的持久化写入。

此前真机曾记录过分钟级 `snapshot_preflight`，因此不能把全部延迟归咎于层级变更代码，也不能在没有本次设备阶段日志时声称已根治分钟级保存问题。

## 改动

### Rust 单节点增量创建

新增 `knowledge_container_create_note_json`，在 Rust 中继续完整验证工作区、命名、父子层级、权限、节点身份冲突和旧版文件夹投影；仅对普通新建返回新增 `NoteEntry` 的规范化 JSON。JNI 仅传回该节点，Android 在原 `updateDataDetailed` 写入锁内追加该节点，避免再生成、回传和解码一份完整工作区数据。

此快速路径仍将当前工作区 JSON 传入 Rust 进行解析，不宣称已实现独立的数据库单行增量事务。它主要减少返回方向的整份数据序列化与解码，以及无关领域在该阶段的对象重建。

### 异常安全回退

如果已有历史孤立子项指向新建节点、父级不可写、身份冲突、原生不可用或单节点解析失败，自动使用原来的完整 Rust 层级操作，不能根据失败的增量结果局部提交。

原 `persistImmediately = true`、`publishAfterDurable = true`、`NonCancellable` 事务保护，以及历史预检、SQLite 提交、恢复镜像、同步数据格式均保留。没有以 UI 提前跳转模拟保存成功，也没有移除任何历史完整性校验。

### 测试与可观测性

Rust 领域规格测试比较根级、旧版文件夹与多层父级的单节点增量重建与完整变更结果，另测重复 ID、无效位置、空文件夹标题、非创建操作及历史孤立子项。源码生成测试锁定完整路径回退与耐久存储标志，并保留阶段日志，新增 `hierarchy_fast_create` 耗时项。

GitHub Actions 使用旧有真实签名构建 Android 3.0.2.1 APK，通过 Rust 编写的 ADB 驱动，在 API 35 模拟器启动应用、打开知识目录、输入新页面标题、点击确定、等待保存完成、强制停止进程并重启检查页面仍在。保存截图、无障碍树与报告。只有相同提交的签名构建和模拟器两项成功，才能称为 CI 验收通过。

## 不解决的边界与后续衡量

本版不改变保存队列 FIFO、全量历史验证和整份 SQLite 快照提交，因此大数据下首次持久化写入仍可能慢。真实重负载性能必须依照 `explicit.queue_wait`、`draft_write_lock`、`hierarchy_encode`、`hierarchy_fast_create`、`snapshot_preflight`、`preflight_native_verification`、`sqlite_commit` 及 `recovery_mirror` 的同次设备记录判定。当前分支模拟器空工作区成功只能说明创建、保存、重启功能可用，不构成数千历史快照下的性能证明。

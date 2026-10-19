# Android 2.23.2.44 任务助手工作区

versionCode 22313。Android 独立版本只递增第四段，Windows 版本不变。

任务 Agent 改为写任务、选资料、核对授权、执行与结果四个步骤。先写目标和选择任务模板，再搜索资料，不再把资料长列表放在输入目标前。底部固定下一步、授权、取消和保存操作。

资料支持按标题和文档库搜索，批量选入保留已有选择，总量限制30条。完整资料、接收方、模型和深度复核设置集中在核对页，必须明确勾选授权才能运行；不可读取的正文不发送并提示数量。

结果默认展示Markdown草稿，按需切换编辑，执行计划、工具记录与核验详情可展开查看。沿用原请求、取消、账户隔离、深度复核与保存核验，不自动修改原始资料。编辑后的草稿会说明核验覆盖范围。

保存后可直接开始新任务。新任务使用独立边界，旧请求、旧草稿和已关闭的保存状态不能沿用；运行和保存未结束前禁止重置。失败可保留任务目标、资料选择和复核选项重试。退出或重做未保存草稿前确认放弃。

本地完整源生成变换链与58项模板和数据保护检查通过。云端147项源生成测试、2项版本身份测试、2项对话上下文测试通过，Kotlin与Java编译、release打包成功；486项JVM测试全部通过，失败、错误和跳过均为0，包含新增的搜索选择与连续任务边界检查。lint为0错误、21项警告。未完成真机键盘、实际模型任务及账户同步验收。

本轮在云端操作，未核对原Windows电脑后台任务和本地、公网服务健康，没有修改同步发现身份或服务配置。

交付包为PR #39合并前测试版，构建提交`1ae7d8cfbee93cdea9732f1a3b04c4bc4d1e740b`。

- [构建与检查记录](https://github.com/WehaoColion/TenRate/actions/runs/39171887015)
- [GitHub Release](https://github.com/WehaoColion/TenRate/releases/tag/v2.23.2.44)
- [下载APK](https://github.com/WehaoColion/TenRate/releases/download/v2.23.2.44/tenfold_v2.23.2.44.apk)
- [SHA256文件](https://github.com/WehaoColion/TenRate/releases/download/v2.23.2.44/tenfold_v2.23.2.44.apk.sha256)

APK大小25,096,877字节，SHA256：`2d2f671893330071914ded15ec31d877e46e3badf309c34af111b5878a72f0e1`。根目录、APK目录和release输出目录三份哈希一致。

包名`com.ofairyo.gridtimer`，版本2.23.2.44 / 22313；原证书SHA256为`4ac7b86a600bbb3215a6187018c1442efcf3ffb25cdbeea0082cab81ef4d605f`，v2/v3签名存在，CI已通过apksigner核验。下载后再次核对证书、版本、arm64-v8a/armeabi-v7a/x86_64三个原生库及新版工作区标记；同步发现公钥、主题、协议与引导地址沿用既有基线。

交付时间：2026-10-19 06:45（北京时间）。GitHub已公开预发布APK及SHA256，发布资产摘要与经过验证的构建产物一致。

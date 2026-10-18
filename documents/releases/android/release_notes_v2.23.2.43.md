# Android 2.23.2.43 — 连续 AI 对话候选

versionCode 22312。Android 独立版本递增第四段，Windows 不变。

- AI 问答改为接近全屏的消息界面，输入和发送固定在底部，配置入口压缩为一行。
- 成功后保留问答消息并清空输入框，可直接继续追问；每条回答独立复制、保存知识页及查看来源。
- 请求带最近最多8轮、24000字符的完整成功问答，问题保持1000字符限制；不裁剪半轮消息。超出上下文窗口的旧消息仍在当前面板显示。
- 请求失败或取消时保留待发问题；未完成、过期及跨账户响应不加入历史。
- 模式、资料范围、AI配置、账户或工作区切换时清空对话上下文。直接问AI不附加知识资料，知识库追问继续按本轮来源引用。
- 当前会话只保留在打开的面板内，关闭后不持久保存；任务Agent继续采用既有资料预览、授权、取消和核验流程。

本地检查：上下文请求的角色顺序、边界与异常输入测试2项通过；模板及既有数据保护检查58项通过；实际源生成变换链通过。
云端检查已完成：147项源生成测试、2项版本身份测试、2项对话上下文测试、484项JVM测试全部通过。Kotlin与Java编译、release打包成功；lint为0错误、21项警告。不声称已完成真机键盘、多轮实际模型请求或账户同步验收。

交付时间：2026-10-19 04:43（北京时间）。构建提交：`0c49589dde3b1c23180edb187bf34bccda7c8a4d`。

- [签名构建与检查记录](https://github.com/WehaoColion/TenRate/actions/runs/39162549712)
- [GitHub Release（PR合并前测试版）](https://github.com/WehaoColion/TenRate/releases/tag/v2.23.2.43)
- [下载APK](https://github.com/WehaoColion/TenRate/releases/download/v2.23.2.43/tenfold_v2.23.2.43.apk)
- [SHA256文件](https://github.com/WehaoColion/TenRate/releases/download/v2.23.2.43/tenfold_v2.23.2.43.apk.sha256)

APK大小25,076,841字节，SHA256：`50c5d6c8a027733c47da515bf27fcf4331ac39e38d208723f92a8e928074e7b2`。GitHub发布资产摘要与构建产物一致，根目录、APK目录与release输出目录三份哈希一致，旧2.23.2.42已归档到忽略的old_apks。

包名`com.ofairyo.gridtimer`；原签名证书SHA256：`4ac7b86a600bbb3215a6187018c1442efcf3ffb25cdbeea0082cab81ef4d605f`；v2/v3签名存在，支持arm64-v8a、armeabi-v7a、x86_64。下载后核对版本、证书、三个原生库与对话标记；同步发现公钥、主题、协议和引导地址与原包基线一致。

当前为云端环境，无法核对用户原Windows电脑的后台任务和本地/公网服务健康，本轮没有修改同步服务或同步发现身份。

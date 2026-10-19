# Android 2.23.2.45

versionCode 22314。Android 仅递增第四段，Windows 版本不变。

风控页改为记账优先：顶部固定“记支出、记收入”，滚动账目时仍可直接开始新增。快捷记账明确记入当天日期，使用原有收入、支出编辑器；返回或系统返回前确认放弃未保存内容。历史日期继续从日账选择。默认打开日账，日、月、季、年入口采用紧凑控件。

财务风险总览、核对、预测与法律分析从固定的“风险分析”入口进入；导出、合并恢复、恢复前快照分享和个人计划从“更多”进入。原核对写入、授权发送与安全合并回调原样保留。当日统计、预算以及分类目标设置按需展开，保留展开动画。法律证据回到所选日期或月份时定位到新的账目位置。

快捷新增复用原条目追加、精确分金额与持久化回执，保存财务变化后使当日核对失效，保留其他账目、删除标记、备注和月快照。账户切换重新建立页面状态。未更改同步发现身份、数据格式、签名证书及 Windows 版本。

本地完整实际源生成链与58项模板和数据保护检查通过。云端147项源生成测试、2项产品版本身份测试、2项对话上下文测试通过；Kotlin、Java编译与release打包成功。489项JVM测试全部通过，失败、错误、跳过均为0，含新增快捷收入精确分金额、快捷支出保留数据和默认日账测试。lint为0错误、21项警告。未完成真机键盘、页面视觉、实际账户同步和原 Windows 电脑后台任务及本地、公网服务健康验收。

交付包为 PR #40 合并前测试版，构建提交 `aeb533a2ff0d196c16eba3e516cb7f691834bb3e`。

- [构建与检查记录](https://github.com/WehaoColion/TenRate/actions/runs/39186336107)
- [GitHub Release](https://github.com/WehaoColion/TenRate/releases/tag/v2.23.2.45)
- [下载 APK](https://github.com/WehaoColion/TenRate/releases/download/v2.23.2.45/tenfold_v2.23.2.45.apk)
- [SHA256 文件](https://github.com/WehaoColion/TenRate/releases/download/v2.23.2.45/tenfold_v2.23.2.45.apk.sha256)

APK大小25,095,201字节，SHA256：`baa3a63cbe754aab75c214efd7cd9cbafdba7693cfc68f82dc65a5aab5d79cd7`。项目根目录、APK目录和release输出目录三份哈希一致，旧APK已归档。

包名 `com.ofairyo.gridtimer`，版本2.23.2.45 / 22314。原签名证书SHA256为 `4ac7b86a600bbb3215a6187018c1442efcf3ffb25cdbeea0082cab81ef4d605f`；CI通过apksigner核验，下载后再次核对版本、证书、v2/v3签名、三个原生架构库、DEX内新记账入口及原同步发现身份，均匹配。GitHub发布资产摘要与验证后的原始构建一致。

交付时间：2026-10-19 10:02（北京时间）。

#!/usr/bin/env bash
# Apply simplified Chinese at the actual Android resource level on the API35 emulator.
set -uo pipefail
PKG="com.ofairyo.gridtimer"
LOG="feature_tour_evidence/中文模式设置记录.txt"
mkdir -p feature_tour_evidence
exec > >(tee "$LOG") 2>&1

echo "中文验收：必须同时检查系统活动配置和应用真实 UI。"
echo "初始活动配置："
adb shell am get-config || true
echo "启用 emulator 调试 root："
adb root || true
adb wait-for-device
adb shell id || true

echo "写入持久系统语言 zh-CN 并完整重启模拟器系统"
adb shell setprop persist.sys.locale zh-CN
adb shell setprop persist.sys.language zh || true
adb shell setprop persist.sys.country CN || true
adb shell settings put system system_locales zh-CN || true
adb reboot || true
adb wait-for-device
success=0

for i in $(seq 1 90); do
  config="$(adb shell am get-config 2>&1 | tr -d '\r')"
  boot="$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')"
  if ((i % 5 == 1)); then
    echo "第 $i 轮：sys.boot_completed=$boot；活动区域配置：${config:0:230}"
  fi
  # Services must really be alive; boot_completed can remain stale after stop/start.
  if [[ "$boot" == "1" ]] && printf '%s' "$config" | grep -Eq '(^|[^a-zA-Z])zh([_-]|-r)CN|(^|[^a-zA-Z])zh([^-a-zA-Z]|$)'; then
    success=1
    break
  fi
  sleep 3
done
echo "最终系统活动配置："
adb shell am get-config || true
echo "persist.sys.locale=$(adb shell getprop persist.sys.locale | tr -d '\r')"
if [[ "$success" != "1" ]]; then
  echo "Android framework 无法切到 zh-CN，禁止截图冒充中文。" >&2
  exit 34
fi
echo "系统中文生效，现在指定 App 区域并重新启动："
adb root || true
adb wait-for-device
adb shell cmd locale set-app-locales "$PKG" --user 0 --locales zh-CN || true
adb shell am force-stop "$PKG" || true
adb shell cmd locale get-app-locales "$PKG" --user 0 || true
echo "系统与独立应用语言请求已提交，后续由 UI 中文文字和四模块校验判定最终是否通过。"

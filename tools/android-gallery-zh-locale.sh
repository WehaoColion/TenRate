#!/usr/bin/env bash
# Force actual Android 35 runtime resources to simplified Chinese, not merely Settings DB.
# Google's Android developer documentation documents setprop + stop/start for emulator.
set -uo pipefail
PKG="com.ofairyo.gridtimer"
LOG="feature_tour_evidence/中文模式设置记录.txt"
mkdir -p feature_tour_evidence
exec > >(tee "$LOG") 2>&1

echo "目标：系统区域 zh-CN；应用区域 zh-CN；界面必须中文。"
echo "改变前正在生效的系统配置："
adb shell am get-config || true
echo "ADB 设备调试权限："
adb root || true
adb wait-for-device
adb shell id || true

echo "按照 Android 官方方法修改 persist.sys.locale 并重启 Android framework："
adb shell setprop persist.sys.locale zh-CN || true
adb shell setprop persist.sys.language zh || true
adb shell setprop persist.sys.country CN || true
adb shell 'stop; sleep 5; start' || true

adb wait-for-device
for i in $(seq 1 90); do
  completed="$(adb shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')"
  if [[ "$completed" == "1" ]]; then break; fi
  sleep 2
done
adb shell input keyevent 82 || true
echo "设置应用独立区域："
adb shell cmd locale set-app-locales "$PKG" --user 0 --locales zh-CN || true
adb shell am force-stop "$PKG" || true
sleep 2

echo "设置后实际 Android 配置："
current_config="$(adb shell am get-config 2>&1 | tr -d '\r')"
printf '%s\n' "$current_config"
echo "系统属性 persist.sys.locale：$(adb shell getprop persist.sys.locale | tr -d '\r')"
echo "应用独立区域："
adb shell cmd locale get-app-locales "$PKG" --user 0 || true

if ! printf '%s' "$current_config" | grep -Eq '(^|[^a-zA-Z])zh([_-]|-r)CN|(^|[^a-zA-Z])zh([^-a-zA-Z]|$)'; then
  echo "中文系统区域未在 am get-config 中生效。严格中止，不交付英文截图。" >&2
  exit 34
fi
echo "已确认实际系统区域为中文。接下来必须继续用界面层验证中文。"

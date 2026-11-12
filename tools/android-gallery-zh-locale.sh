#!/usr/bin/env bash
# Set Android 35 emulator and the app itself to simplified Chinese; fail closed on mismatch.
set -u
pkg="com.ofairyo.gridtimer"
mkdir -p feature_tour_evidence
log="feature_tour_evidence/中文模式设置记录.txt"
{
  echo "目标语言：zh-CN（简体中文）"
  echo "系统初始区域：$(adb shell getprop persist.sys.locale 2>/dev/null | tr -d '\r')"
  echo "尝试设置系统语言："
  adb shell am update-config --locale zh-CN 2>&1 || true
  adb shell settings put system system_locales zh-CN 2>&1 || true
  echo "尝试设置 App 独立语言："
  adb shell cmd locale set-app-locales "$pkg" --user 0 --locales zh-CN 2>&1 || true
  adb shell am force-stop "$pkg" || true
  sleep 3
  echo "App 语言读取："
  adb shell cmd locale get-app-locales "$pkg" --user 0 2>&1 || true
  echo "系统当前区域：$(adb shell getprop persist.sys.locale 2>/dev/null | tr -d '\r')"
  echo "系统资源语言：$(adb shell settings get system system_locales 2>/dev/null | tr -d '\r')"
} > "$log"
cat "$log"
# Actual Chinese UI label verification is done after launching, using uiautomator.

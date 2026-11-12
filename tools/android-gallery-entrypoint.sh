#!/usr/bin/env bash
# Run feature tour inside the booted GitHub Android emulator.
set -uo pipefail

mkdir -p feature_tour_evidence/截图/工作台

if ! adb install -r signed_apk/tenfold_v2.13.5.1.apk > apk_install_log.txt 2>&1; then
  printf 'APK 安装失败；详见 apk_install_log.txt\n' > gallery_status.txt
  cp apk_install_log.txt feature_tour_evidence/安装日志.txt
  exit 1
fi

adb shell am start -W -n com.ofairyo.gridtimer/.MainActivity > app_launch_log.txt 2>&1 || true
sleep 3
adb exec-out screencap -p > feature_tour_evidence/截图/工作台/0000_模拟器实际启动.png || true

timeout 80m python3 -u tools/android_feature_tour.py > gallery_tour_log.txt 2>&1
tour_code=$?

adb shell pm clear com.ofairyo.gridtimer > app_clear_log.txt 2>&1 || true
timeout 35m ./android_customer_canvas_probe customer_canvas_evidence > canvas_probe_log.txt 2>&1
canvas_code=$?

printf '全面截图脚本退出码: %s\n客户推荐画布验收退出码: %s\n' "$tour_code" "$canvas_code" | tee gallery_status.txt
if [[ "$tour_code" -eq 0 && "$canvas_code" -eq 0 ]]; then
  echo 'Both probes completed; screenshot count and ZIP contents require separate validation.'
  exit 0
fi
printf '至少一项验收失败，请检查日志和截图，不可报告全部通过。\n' >> gallery_status.txt
exit 1

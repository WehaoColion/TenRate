#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p Native/iphoneos Native/iphonesimulator build
rustup target add aarch64-apple-ios aarch64-apple-ios-sim
cargo test --locked --manifest-path native/Cargo.toml --test product_flows
for pair in 'aarch64-apple-ios iphoneos' 'aarch64-apple-ios-sim iphonesimulator'; do
  read -r target platform <<< "$pair"
  cargo build --locked --release --manifest-path native/Cargo.toml --target "$target"
  cp "native/target/$target/release/libtenrate_ios_core.a" "Native/$platform/"
done
xcodegen generate
xcodebuild -project TenRate.xcodeproj -scheme TenRate -configuration Release \
  -sdk iphoneos -destination 'generic/platform=iOS' -derivedDataPath build/device \
  CODE_SIGNING_ALLOWED=NO build

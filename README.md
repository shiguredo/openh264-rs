# openh264-rs

[![shiguredo_openh264](https://img.shields.io/crates/v/shiguredo_openh264.svg)](https://crates.io/crates/shiguredo_openh264)
[![Documentation](https://docs.rs/shiguredo_openh264/badge.svg)](https://docs.rs/shiguredo_openh264)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

## About Shiguredo's open source software

We will not respond to PRs or issues that have not been discussed on Discord. Also, Discord is only available in Japanese.

Please read <https://github.com/shiguredo/oss> before use.

## 時雨堂のオープンソースソフトウェアについて

利用前に <https://github.com/shiguredo/oss> をお読みください。

## 概要

Cisco の [OpenH264](https://github.com/cisco/openh264) を Rust から利用するためのバインディングライブラリです。

## 特徴

- H.264 エンコーダー/デコーダーの安全な Rust ラッパー
- 実行時に OpenH264 共有ライブラリを動的ロード (`dlopen` / `LoadLibraryW`)
  - ビルド時のリンク不要
- ランタイム依存は `log` クレートのみ
- macOS / Linux / Windows 対応
- ビルド時に [bindgen](https://github.com/rust-lang/rust-bindgen) で C ヘッダーからバインディングを自動生成

## OpenH264 について

OpenH264 は Cisco がオープンソースで公開している H.264 コーデックライブラリです。
バイナリモジュールは Cisco が提供しており、利用者が自分でダウンロードして使用する必要があります。

- <https://github.com/cisco/openh264>

## 使い方

### ライブラリのロード

```rust
use shiguredo_openh264::Openh264Library;

let lib = Openh264Library::load("/path/to/libopenh264.so")?;
println!("OpenH264 version: {}", lib.runtime_version());
```

### デコード

```rust
use shiguredo_openh264::{Openh264Library, Decoder};

let lib = Openh264Library::load("/path/to/libopenh264.so")?;
let mut decoder = Decoder::new(lib)?;

// Annex.B 形式の H.264 データをデコード
if let Some(frame) = decoder.decode(&h264_data)? {
    let y = frame.y_plane();
    let u = frame.u_plane();
    let v = frame.v_plane();
    let width = frame.width();
    let height = frame.height();
}
```

### エンコード

```rust
use shiguredo_openh264::{Openh264Library, Encoder, EncoderConfig};

let lib = Openh264Library::load("/path/to/libopenh264.so")?;
let config = EncoderConfig {
    width: 1920,
    height: 1080,
    target_bitrate: 2_000_000,
    ..Default::default()
};
let mut encoder = Encoder::new(lib, &config)?;

// I420 形式の YUV データをエンコード
if let Some(frame) = encoder.encode(&y_data, &u_data, &v_data)? {
    let is_keyframe = frame.keyframe;
    let compressed = &frame.data;
}
```

## ライセンス

Apache License 2.0

```text
Copyright 2026-2026, Shiguredo Inc.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

## OpenH264

<https://www.openh264.org/BINARY_LICENSE.txt>

```text
"OpenH264 Video Codec provided by Cisco Systems, Inc."
```

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
- 入出力フォーマットは I420 (YUV 4:2:0 planar) 固定 (OpenH264 の仕様)
- 実行時に OpenH264 共有ライブラリを動的ロード (`dlopen` / `LoadLibraryW`)
  - ビルド時のリンク不要
- ランタイム依存クレートなし
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

### コーデック対応情報の取得

ライブラリがサポートするコーデックの情報（デコード/エンコード対応、プロファイル）を取得できます。

```rust
use shiguredo_openh264::{EncodingProfiles, Openh264Library};

let lib = Openh264Library::load("/path/to/libopenh264.so")?;
let info = lib.supported_codecs();

println!("デコード対応: {}", info.decoding.supported);
println!("エンコード対応: {}", info.encoding.supported);

if let EncodingProfiles::H264(profiles) = &info.encoding.profiles {
    println!("対応プロファイル: {:?}", profiles);
}
```

### エンコード

入力は I420 (YUV 4:2:0 planar) 形式で、Y, U, V プレーンを個別に渡します。
出力は Annex.B 形式の H.264 データです。

```rust
use shiguredo_openh264::{EncodeOptions, Encoder, EncoderConfig, FrameType, Openh264Library};

let lib = Openh264Library::load("/path/to/libopenh264.so")?;
let config = EncoderConfig::new(1920, 1080, 2_000_000, 30, 1);
let mut encoder = Encoder::new(lib, config)?;

// I420 形式の YUV データをエンコード
if let Some(frame) = encoder.encode(&y_data, &u_data, &v_data, &EncodeOptions::default())? {
    let compressed = &frame.data;

    // IDR フレーム時のみ SPS/PPS が含まれる
    if frame.frame_type == FrameType::Idr {
        let sps = &frame.sps_list[0];
        let pps = &frame.pps_list[0];
    }
}
```

オプションを指定する場合:

```rust
use shiguredo_openh264::{EncoderConfig, Profile, RateControlMode};

let config = EncoderConfig {
    profile: Some(Profile::Main),
    rate_control_mode: Some(RateControlMode::Bitrate),
    ..EncoderConfig::new(1920, 1080, 2_000_000, 30, 1)
};
```

### デコード

入力は Annex.B 形式の H.264 データで、出力は I420 (YUV 4:2:0 planar) 形式です。

```rust
use shiguredo_openh264::{Decoder, Openh264Library};

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

// 残りのフレームをフラッシュ
if let Some(frame) = decoder.finish()? {
    // ...
}
```

## 動的パラメーター変更

WebRTC やアダプティブビットレートストリーミングなど、ストリーム中にパラメーターが変わるユースケースに対応しています。

### エンコーダー

`set_resolution()` / `set_bitrate()` / `set_frame_rate()` で個別に変更できます。エンコーダーの作り直しは不要です。

```rust
// 解像度を変更
encoder.set_resolution(1280, 720)?;

// ビットレートを変更
encoder.set_bitrate(1_000_000)?;

// フレームレートを変更
encoder.set_frame_rate(60, 1)?;
```

全パラメーターを一括で変更する場合は `set_config()` を使用します。

```rust
encoder.set_config(EncoderConfig::new(1280, 720, 1_000_000, 60, 1))?;
```

### デコーダー

利用者側の操作は不要です。ストリーム中に解像度が変わった場合、OpenH264 内部で自動的に対応されます。

`DecodedFrame` はフレームごとに `width()` / `height()` を持っているので、フレームごとにサイズを確認してください。

```rust
// 解像度が変わっても同じデコーダーで継続可能
if let Some(frame) = decoder.decode(&data_1080p)? {
    assert_eq!(frame.width(), 1920);
}

if let Some(frame) = decoder.decode(&data_720p)? {
    assert_eq!(frame.width(), 1280);  // 自動的に変更される
}
```

### まとめ

| | エンコーダー | デコーダー |
|---|---|---|
| 仕組み | `set_resolution()` 等で明示的に変更 | OpenH264 が自動検出 |
| 利用者の操作 | 変更メソッドを呼ぶ | 不要 |
| 制約 | なし | なし |

## テスト

テストの実行には OpenH264 共有ライブラリが必要です。
環境変数 `OPENH264_PATH` にライブラリのパスを指定してください。

```bash
OPENH264_PATH=/path/to/libopenh264.dylib cargo test
```

## サンプル

`examples/animate.rs` は raden で描画したアニメーションを OpenH264 でエンコードし MP4 ファイルに出力するサンプルです。

```bash
OPENH264_PATH=/path/to/libopenh264.dylib cargo run --example animate
```

実行すると `output.mp4` が生成されます。

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

# 公開 API の数値入力が境界チェックなしで FFI に渡される

Created: 2026-03-31
Model: Opus 4.6

## 概要

`EncoderConfig` の `usize` / `NonZeroUsize` フィールドおよび `set_bitrate()` / `set_resolution()` の引数が、`as` キャストで FFI 側の `c_int` / `c_ushort` / `c_uint` に変換されている。64-bit 環境では `usize` は 64 ビットだが、キャスト先はそれより小さいため、範囲外の値がサイレントに切り詰められて OpenH264 に渡る。

## 根拠

`as` キャストは Rust の仕様上パニックせず、上位ビットを切り捨てる。これにより:

- `width: usize` が `c_int` (i32) の範囲を超えると負値や小さい値に化ける
- `thread_count: NonZeroUsize` が `c_ushort` (u16) の 65,535 を超えると切り詰められる
- `target_bitrate: usize` が `c_int` の範囲を超えると負値になる

利用者が公開 API 経由で設定可能な値なので、FFI 境界での入力バリデーションが必須。

## 影響箇所

### `validate_config()` で検証すべきフィールド

| フィールド | Rust 型 | FFI 先の型 | 必要な上限 |
|---|---|---|---|
| `width` | `usize` | `c_int` (i32) | `i32::MAX` |
| `height` | `usize` | `c_int` (i32) | `i32::MAX` |
| `target_bitrate` | `usize` | `c_int` (i32) | `i32::MAX` |
| `ref_frame_count` | `NonZeroUsize` | `c_int` (i32) | `i32::MAX` |
| `thread_count` | `NonZeroUsize` | `c_ushort` (u16) | `u16::MAX` |
| `spatial_layers` | `NonZeroUsize` | `c_int` (i32) | `i32::MAX` |
| `temporal_layers` | `NonZeroUsize` | `c_int` (i32) | `i32::MAX` |
| `intra_period` | `usize` | `c_uint` (u32) | `u32::MAX` |
| `SliceMode::FixedCount` | `usize` | `c_uint` (u32) | `u32::MAX` |
| `SliceMode::SizeConstrained` | `usize` | `c_uint` (u32) | `u32::MAX` |

### `validate_config()` を経由しないメソッド

| メソッド | 引数 | FFI 先の型 | 必要な上限 |
|---|---|---|---|
| `set_bitrate(bitrate)` | `usize` | `c_int` (i32) | `i32::MAX` |
| `set_resolution(width, height)` | `usize` | `c_int` (i32) | `i32::MAX` |

## 対応方針

1. `validate_config()` に各フィールドの上限チェックを追加する
2. `set_bitrate()` と `set_resolution()` にも同様の上限チェックを追加する
3. 範囲外の場合は `Error::InvalidParameter` を返す

Completed: 2026-03-31

## 解決方法

`validate_config()` に `c_int::try_from()` / `c_ushort::try_from()` / `c_uint::try_from()` による上限チェックを追加した。対象フィールド: `width`, `height`, `target_bitrate`, `ref_frame_count`, `thread_count`, `spatial_layers`, `temporal_layers`, `intra_period`, `SliceMode::FixedCount`, `SliceMode::SizeConstrained`。

`validate_config()` を経由しない `set_bitrate()` と `set_resolution()` にも同様の `c_int::try_from()` チェックを追加した。

範囲外の値が渡された場合は `Error::InvalidParameter` を返し、サイレントな切り詰めを防止する。

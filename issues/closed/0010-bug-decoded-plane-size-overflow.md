# デコード面サイズ計算が usize オーバーフローで panic し得る

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`DecodedFrame::from_buffer_info()` は寸法とストライドの符号は検証しているが、`height * y_stride` と `uv_height * u_stride` は通常の乗算のまま。FFI 側が極端に大きい正値を返すと debug ビルドではオーバーフロー panic し、release ではラップ後に不正な長さで `from_raw_parts` へ進み未定義動作になる。

## 根拠

`iWidth`, `iHeight`, `iStride[*]` はすべて `c_int` (i32) で最大値は 2,147,483,647。正値の検証を通過しても `height=2147483647 * y_stride=2 = 4294967294` で `usize` オーバーフロー（32-bit 環境）や、さらに大きい組み合わせで 64-bit 環境でもオーバーフローする可能性がある。

## 解決方法

`height * y_stride` / `uv_height * u_stride` / `uv_height * v_stride` を `checked_mul` に変更し、オーバーフロー時は `Error::InvalidParameter` を返すようにした。

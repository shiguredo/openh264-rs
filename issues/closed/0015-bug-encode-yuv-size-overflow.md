# encode() の入力サイズ計算がオーバーフローで panic し得る

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`encode()` の `y_size = height * stride` と `u_size = height.div_ceil(2) * stride` が通常乗算のまま。極端な解像度では debug ビルドで panic し、release ではラップして誤った `InvalidYuvSize` 判定になる。

## 根拠

デコード側 (`DecodedFrame::from_buffer_info()`) では同じパターンを `checked_mul` で防御済み。エンコード側も同じ水準の防御が必要。特に 32-bit 環境では `c_int::MAX` 同士の乗算で `usize` オーバーフローが起きる。

## 解決方法

`height * stride` を `height.checked_mul(stride)` に変更し、オーバーフロー時は `Error::InvalidYuvSize` を返すようにした。

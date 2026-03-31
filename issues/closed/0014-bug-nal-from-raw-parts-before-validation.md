# NAL 末尾境界を確認する前に from_raw_parts している

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`offset` の `checked_add` が `from_raw_parts` の後に配置されていたため、`nal_len` が異常に大きい場合に先に範囲外スライスを作ってしまう。検証は `from_raw_parts` の前に行う必要がある。

## 根拠

`from_raw_parts(layer_info.pBsBuf.add(offset), nal_len)` は `offset + nal_len` 分のメモリにアクセスする。この合計が妥当かを検証する `checked_add` がその後に配置されていると、不正な長さのスライスが先に作られて未定義動作になる。

## 解決方法

`next_offset = offset.checked_add(nal_len)` を `from_raw_parts` の前に移動し、オーバーフロー時はエラーを返すようにした。`from_raw_parts` は検証通過後にのみ実行し、ループ末尾で `offset = next_offset` に更新する。

# NAL オフセット加算が未検証でオーバーフローし得る

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`encode()` の NAL 処理ループで `offset += nal_len` が通常加算のまま。FFI 側が大きい NAL 長を返すと、debug ビルドではオーバーフロー panic、release ではラップして `pBsBuf.add(offset)` が誤った位置を指す。

## 根拠

`iNalCount` 上限が 65536、各 `nal_len` は `c_int` の正値（最大 `i32::MAX` ≒ 2G）なので、理論上 `65536 * 2G` で 64-bit `usize` でもオーバーフローする可能性がある。

## 解決方法

`offset += nal_len` を `offset.checked_add(nal_len)` に変更し、オーバーフロー時は `Error::InvalidParameter` を返すようにした。

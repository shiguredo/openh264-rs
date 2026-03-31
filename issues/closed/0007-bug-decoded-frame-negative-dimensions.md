# デコード結果の寸法・ストライドを符号なしへ無条件キャストしている

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`DecodedFrame::from_buffer_info()` は `iWidth` / `iHeight` / `iStride[*]` (すべて `c_int`) を負値チェックなしで `as usize` に変換している。FFI 側の異常で負値が返ると巨大な `usize` になり、`height * stride` のオーバーフローで panic するか、`from_raw_parts` に巨大長を渡して未定義動作になる。

## 根拠

`c_int` の負値を `as usize` でキャストすると、例えば `-1` は `usize::MAX` (18446744073709551615) になる。これに任意の正値を掛けると確実にオーバーフローする。NULL チェックだけではこのパスは防げない。

## 解決方法

`from_buffer_info()` で `iWidth`, `iHeight`, `iStride[0]`, `iStride[1]` がすべて正値であることを `as usize` 変換前に検証するようにした。いずれかが 0 以下の場合は `Error::InvalidParameter` を返す。

# FFI 出力のポインタを NULL チェックせず from_raw_parts している

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

デコード済みフレームの `pDst[0..2]` とエンコード済みフレームの `pNalLengthInByte` / `pBsBuf` を NULL チェックなしで `from_raw_parts` に渡している。FFI 側の異常状態でこれらが NULL のまま返された場合、未定義動作やセグフォが発生する。

## 根拠

- `DecodedFrame::from_buffer_info()`: `iBufferStatus == 1` かつ I420 でも `pDst` の各プレーンが NULL の可能性を排除できない
- エンコード出力: `iNalCount > 0` でも `pNalLengthInByte` / `pBsBuf` が NULL の可能性がある。既にコメントで「不正なアドレスを指していてクラッシュする可能性がある」と認識されている

## 解決方法

- `DecodedFrame::from_buffer_info()` を `Result<Self, Error>` に変更し、`pDst[0..2]` のいずれかが NULL の場合は `Error::InvalidParameter` を返すようにした
- エンコード出力のループで `iNalCount > 0` かつ `pNalLengthInByte.is_null() || pBsBuf.is_null()` の場合は `continue` でスキップするようにした

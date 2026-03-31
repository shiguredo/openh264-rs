# WelsCreate* 成功時の NULL ハンドルを検証していない

Created: 2026-03-31
Completed: 2026-03-31
Model: Opus 4.6

## 概要

`Decoder::new()` と `Encoder::new()` は `WelsCreateDecoder` / `WelsCreateSVCEncoder` の戻り値コードしか見ておらず、生成されたハンドルが NULL かどうかを確認しないまま `(**inner)` を即座に参照している。FFI 側が `code == 0` なのに NULL を返した場合、セグフォが発生する。

## 根拠

`is_decoder_available()` / `is_encoder_available()` では `code != 0 || inner.is_null()` で防御しているが、コンストラクターには同じ防御がない。FFI 境界では戻り値コードとポインタの両方を検証するのが原則。

## 解決方法

`Decoder::new()` と `Encoder::new()` の `Error::check(code, name)?` の直後に `inner.is_null()` チェックを追加した。NULL の場合は `Error::Openh264Error { code: -1, function: name }` を返す。

# fuzzing ターゲットを追加する

Created: 2026-03-31
Model: Opus 4.6

## 概要

公開 API に対する fuzzing ターゲットがまだ存在しない。FFI 境界の数値範囲チェックを追加したが、バリデーションの漏れがないことを fuzzing で検証すべき。

## 根拠

- バリデーションは人間が書いたものなので、見落としがある可能性がある
- OpenH264 自体がバリデーションを通過した値でクラッシュしないことも確認が必要
- CLAUDE.md のテスト方針で fuzzing は「任意入力に対するクラッシュ耐性（パニック安全性）」を担当する

## 追加すべき fuzz ターゲット

### 1. エンコーダー初期化 (`fuzz_encoder_new`)

任意の `EncoderConfig` を生成して `Encoder::new()` に渡し、`Ok` か `Err` のどちらかを返してパニックしないことを確認する。

### 2. エンコーダー動的パラメーター変更 (`fuzz_encoder_set_options`)

有効なエンコーダーを生成した後、任意の値で `set_bitrate()` / `set_resolution()` / `set_frame_rate()` / `set_config()` を呼び、パニックしないことを確認する。

### 3. デコーダー (`fuzz_decoder_decode`)

任意のバイト列を `Decoder::decode()` に渡し、OpenH264 デコーダーがパニック・クラッシュしないことを確認する。

## 備考

- cargo-fuzz を使用する
- `OPENH264_PATH` 環境変数でライブラリパスを指定する必要がある

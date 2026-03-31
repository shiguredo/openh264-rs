# 変更履歴

- UPDATE
  - 後方互換がある変更
- ADD
  - 後方互換がある追加
- CHANGE
  - 後方互換のない変更
- FIX
  - バグ修正

## develop


## 2026.1.0

**リリース日**: 2026-03-31

- [ADD] `ComplexityMode` / `RateControlMode` / `SliceMode` enum を追加する
  - @voluntas
- [ADD] `Openh264Library::supported_codecs()` を追加する
  - コーデックのデコード/エンコード対応状況と対応プロファイルを返す
  - OpenH264 は Constrained Baseline Profile のみ対応
  - @voluntas
- [ADD] エンコーダーの動的パラメーター変更メソッドを追加する
  - `set_bitrate()` でビットレートを動的に変更する
  - `set_frame_rate()` でフレームレートを動的に変更する
  - `set_resolution()` で解像度を動的に変更する
  - `set_config()` で全パラメーターを一括変更する
  - @voluntas
- [ADD] `EncodedFrame` に `Clone` derive を追加する
  - @voluntas
- [CHANGE] エンコーダー/デコーダーの API を再設計する
  - `Encoder::new` の第 2 引数を `&EncoderConfig` から `EncoderConfig` (所有) に変更する
  - `Encoder::encode()` に `&EncodeOptions` 引数を追加する
    - `force_idr` フラグで IDR フレームの強制生成が可能になる
  - `EncodedFrame` から `keyframe: bool` を削除し `frame_type: FrameType` に変更する
  - `EncodedFrame` に `sps_list` / `pps_list` フィールドを追加し SPS/PPS を圧縮データから分離する
  - `DecodedFrame` からライフタイムパラメーターを削除し YUV データを所有する形に変更する
  - @voluntas
- [CHANGE] `EncoderConfig` の非必須フィールドを `Option<T>` に変更する
  - `None` で OpenH264 の `GetDefaultParams` のデフォルト値をそのまま使用する
  - `EncoderConfig::new()` コンストラクターを追加する
  - `EncoderConfig` / 各 enum から `Default` 実装を削除する
  - @voluntas
- [CHANGE] ハードコードされていたレベル / エントロピー符号化モードを `EncoderConfig` のフィールドに変更する
  - `Level` / `EntropyCodingMode` enum を追加する
  - `entropy_coding: bool` を `entropy_coding_mode: Option<EntropyCodingMode>` に変更する
  - プロファイルは OpenH264 が `entropy_coding_mode` に基づいて自動選択する
  - @voluntas
- [CHANGE] バージョン不一致時の処理を `log::warn!` からエラーに変更する
  - `Error::VersionMismatch` バリアントを追加する
  - ランタイム依存の `log` クレートを削除する
  - @voluntas
- [FIX] `fps_numerator` / `fps_denominator` がゼロの場合にパニックする問題を修正する
  - `Encoder::new()` / `set_frame_rate()` / `set_config()` でバリデーションを追加する
  - @voluntas
- [FIX] `Decoder::decode()` の入力データサイズが `c_int::MAX` を超える場合のチェックを追加する
  - @voluntas
- [FIX] FFI 境界で `usize` / `NonZeroUsize` から `c_int` / `c_ushort` / `c_uint` への変換時に範囲チェックを追加する
  - `validate_config()` / `set_bitrate()` / `set_resolution()` で `try_from` による検証を行い、範囲外の値は `Error::InvalidParameter` を返す
  - @voluntas

### misc

- [CHANGE] ビルド依存の `toml` クレートを `shiguredo_toml` に置き換える
  - @voluntas


## 2025.1.0

**リリース日**: 2025-09-26

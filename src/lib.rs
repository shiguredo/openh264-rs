//! [OpenH264] の Rust バインディング
//!
//! 対応プロファイルは Constrained Baseline Profile up to Level 5.2。
//! 入出力フォーマットは I420 (YUV 4:2:0 planar) 固定。
//! これは OpenH264 エンコーダー・デコーダー双方の仕様による制約。
//!
//! [OpenH264]: https://github.com/cisco/openh264
#![warn(missing_docs)]

use std::{
    ffi::{c_int, c_longlong, c_uint, c_ushort},
    mem::MaybeUninit,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

mod dl;
mod sys;

/// ビルド時に参照した OpenH264 のバージョン（タグ）
pub const BUILD_VERSION: &str = sys::BUILD_METADATA_VERSION;

/// ビルド時に参照した OpenH264 のリポジトリ URL
pub const BUILD_REPOSITORY: &str = sys::BUILD_METADATA_REPOSITORY;

/// エラー
#[derive(Debug)]
#[allow(missing_docs)]
pub enum Error {
    /// 共有ライブラリ関連のエラー
    SharedLibraryError(String),

    /// openh264 関連のエラー
    Openh264Error { code: c_int, function: &'static str },

    /// openh264 の仮想テーブル (vtbl) のメソッドが None だった場合のエラー
    UnavailableMethod(&'static str),

    /// ビルド時と実行時の OpenH264 バージョンが不一致
    VersionMismatch {
        /// ビルド時のバージョン
        build_version: &'static str,
        /// 実行時のバージョン
        runtime_version: String,
    },

    /// デコード結果が I420 以外だった
    UnsupportedFormat { format: sys::EVideoFormatType },

    /// エンコード時の入力 YUV のサイズが不正だった
    InvalidYuvSize,

    /// パラメーターが不正
    InvalidParameter(String),
}

impl Error {
    fn check(code: c_int, function: &'static str) -> Result<(), Error> {
        match code {
            0 => Ok(()),
            _ => Err(Self::Openh264Error { code, function }),
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::SharedLibraryError(error) => write!(f, "{error}"),
            Error::Openh264Error { code, function } => {
                write!(f, "{function}() failed: code={code}")
            }
            Error::UnavailableMethod(name) => write!(f, "unavailable method: name={name}"),
            Error::VersionMismatch {
                build_version,
                runtime_version,
            } => write!(
                f,
                "OpenH264 version mismatch: build={build_version}, runtime={runtime_version}"
            ),
            Error::UnsupportedFormat { format } => {
                write!(f, "unsupported video format (not I420): format={format}")
            }
            Error::InvalidYuvSize => write!(f, "invalid input YUV size"),
            Error::InvalidParameter(msg) => write!(f, "invalid parameter: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

// ============================================================================
// コーデック対応情報
// ============================================================================

/// コーデック種別
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoCodecType {
    /// H.264
    H264,
}

/// H.264 エンコーディングプロファイル
///
/// OpenH264 は Constrained Baseline Profile のみ対応している (README 参照)。
/// フル Baseline (ASO/FMO/冗長スライス)、Main、High には対応していない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264EncodingProfile {
    /// Constrained Baseline プロファイル
    ConstrainedBaseline,
}

/// コーデック固有のエンコードプロファイル情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodingProfiles {
    /// H.264 プロファイル一覧
    H264(Vec<H264EncodingProfile>),
    /// プロファイル情報なし
    Unsupported,
}

/// デコード対応情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodingInfo {
    /// デコードに対応しているか
    pub supported: bool,
    /// ハードウェアアクセラレーションに対応しているか
    pub hardware_accelerated: bool,
}

/// エンコード対応情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodingInfo {
    /// エンコードに対応しているか
    pub supported: bool,
    /// ハードウェアアクセラレーションに対応しているか
    pub hardware_accelerated: bool,
    /// コーデック固有のプロファイル情報
    pub profiles: EncodingProfiles,
}

/// コーデック対応情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodecInfo {
    /// コーデック種別
    pub codec: VideoCodecType,
    /// デコード情報
    pub decoding: DecodingInfo,
    /// エンコード情報
    pub encoding: EncodingInfo,
}

// ============================================================================

/// H.264 レベル
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Level {
    /// Level 1.0
    L1,
    /// Level 1.1
    L1_1,
    /// Level 1.2
    L1_2,
    /// Level 1.3
    L1_3,
    /// Level 2.0
    L2,
    /// Level 2.1
    L2_1,
    /// Level 2.2
    L2_2,
    /// Level 3.0
    L3,
    /// Level 3.1
    L3_1,
    /// Level 3.2
    L3_2,
    /// Level 4.0
    L4,
    /// Level 4.1
    L4_1,
    /// Level 4.2
    L4_2,
    /// Level 5.0
    L5,
    /// Level 5.1
    L5_1,
    /// Level 5.2
    L5_2,
}

/// エントロピー符号化モード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntropyCodingMode {
    /// CAVLC (Context-Adaptive Variable-Length Coding)
    Cavlc,
    /// CABAC (Context-Adaptive Binary Arithmetic Coding)
    Cabac,
}

/// エンコードされた映像フレームのタイプ
///
/// OpenH264 の `EVideoFrameType` に対応する。
/// `Skip` は `encode()` が `None` を返すケースに対応するので含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    /// IDR フレーム (NAL unit type 5)
    ///
    /// デコーダーがこのポイントから新規デコード開始可能。
    /// SPS/PPS が付随する。
    Idr,

    /// I フレーム (NAL unit type 1, イントラスライス)
    I,

    /// P フレーム (NAL unit type 1, インタースライス)
    P,
}

/// エンコード時のオプション
///
/// フレームごとに異なるオプションを指定可能。
#[derive(Debug, Clone, Default)]
pub struct EncodeOptions {
    /// 次のフレームを強制的に IDR フレームとしてエンコードする
    ///
    /// OpenH264 の `ForceIntraFrame(bIDR=true)` に対応する。
    pub force_idr: bool,
}

/// openh264 用の共有ライブラリを管理するための構造体
#[derive(Debug, Clone)]
pub struct Openh264Library {
    lib: Arc<dl::DynLib>,
    path: PathBuf,
    version: sys::OpenH264Version,
}

impl Openh264Library {
    /// 指定のパスにある動的ライブラリをロードする
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        unsafe {
            let lib = dl::DynLib::open(path.as_ref()).map_err(Error::SharedLibraryError)?;
            let get_version: unsafe extern "C" fn() -> sys::OpenH264Version = lib
                .get(b"WelsGetCodecVersion")
                .map_err(Error::SharedLibraryError)?;
            let version = get_version();
            let this = Self {
                lib: Arc::new(lib),
                path: path.as_ref().to_path_buf(),
                version,
            };
            this.check_version()?;
            Ok(this)
        }
    }

    /// 共有ライブラリのパスを取得する
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 共有ライブラリのバージョンを取得する
    pub fn runtime_version(&self) -> String {
        format!(
            "v{}.{}.{}",
            self.version.uMajor, self.version.uMinor, self.version.uRevision
        )
    }

    fn check_version(&self) -> Result<(), Error> {
        let runtime_version = self.runtime_version();
        if runtime_version != BUILD_VERSION {
            return Err(Error::VersionMismatch {
                build_version: BUILD_VERSION,
                runtime_version,
            });
        }
        Ok(())
    }

    /// 利用可能な H.264 コーデックの対応情報を返す
    ///
    /// OpenH264 はソフトウェアコーデックであるため、`hardware_accelerated` は常に `false` を返す。
    /// プロファイルの検出は各プロファイルでエンコーダーの初期化を試行して判定する。
    pub fn supported_codecs(&self) -> CodecInfo {
        let decoding = DecodingInfo {
            supported: self.is_decoder_available(),
            hardware_accelerated: false,
        };

        let encoder_available = self.is_encoder_available();

        let profiles = if encoder_available {
            EncodingProfiles::H264(self.detect_supported_profiles())
        } else {
            EncodingProfiles::Unsupported
        };

        let encoding = EncodingInfo {
            supported: encoder_available,
            hardware_accelerated: false,
            profiles,
        };

        CodecInfo {
            codec: VideoCodecType::H264,
            decoding,
            encoding,
        }
    }

    /// デコーダーを生成・初期化できるか試行する
    fn is_decoder_available(&self) -> bool {
        let mut inner = std::ptr::null_mut();
        unsafe {
            let Ok(code) = self.call("WelsCreateDecoder", |f: WelsCreateDecoder| f(&mut inner))
            else {
                return false;
            };
            if code != 0 || inner.is_null() {
                return false;
            }

            let param = MaybeUninit::<sys::SDecodingParam>::zeroed();
            let mut param = param.assume_init();
            param.pFileNameRestructed = std::ptr::null_mut();
            param.uiTargetDqLayer = 1;
            param.eEcActiveIdc = sys::ERROR_CON_IDC_ERROR_CON_DISABLE;
            param.bParseOnly = false;
            param.sVideoProperty.eVideoBsType = sys::VIDEO_BITSTREAM_TYPE_VIDEO_BITSTREAM_AVC;

            let supported = match (**inner).Initialize {
                Some(init) => init(inner, &param) == 0,
                None => false,
            };

            if let Some(uninit) = (**inner).Uninitialize {
                uninit(inner);
            }
            let _ = self.call("WelsDestroyDecoder", |f: WelsDestroyDecoder| f(inner));

            supported
        }
    }

    /// エンコーダーを生成できるか試行する
    fn is_encoder_available(&self) -> bool {
        let mut inner = std::ptr::null_mut();
        unsafe {
            let Ok(code) = self.call("WelsCreateSVCEncoder", |f: WelsCreateSVCEncoder| {
                f(&mut inner)
            }) else {
                return false;
            };
            if code != 0 || inner.is_null() {
                return false;
            }
            let _ = self.call("WelsDestroySVCEncoder", |f: WelsDestroySVCEncoder| f(inner));
            true
        }
    }

    /// 各プロファイルでエンコーダーの初期化を試行して対応プロファイルを検出する
    /// OpenH264 は Constrained Baseline Profile のみ対応 (README 参照)。
    /// PRO_MAIN/PRO_HIGH は InitializeExt で受け入れられるが、
    /// 実際の符号化能力は Constrained Baseline + CABAC に留まり、
    /// 8x8 DCT 変換 (High 必須) 等は未実装。
    fn detect_supported_profiles(&self) -> Vec<H264EncodingProfile> {
        let mut profiles = Vec::new();

        if self.is_profile_supported(sys::EProfileIdc_PRO_BASELINE) {
            profiles.push(H264EncodingProfile::ConstrainedBaseline);
        }

        profiles
    }

    /// 指定したプロファイルでエンコーダーを初期化できるか試行する
    fn is_profile_supported(&self, profile_idc: sys::EProfileIdc) -> bool {
        let mut inner = std::ptr::null_mut();
        unsafe {
            let Ok(code) = self.call("WelsCreateSVCEncoder", |f: WelsCreateSVCEncoder| {
                f(&mut inner)
            }) else {
                return false;
            };
            if code != 0 || inner.is_null() {
                return false;
            }

            let result = (|| {
                let mut param = MaybeUninit::<sys::SEncParamExt>::zeroed();
                let get_default = (**inner).GetDefaultParams?;
                if get_default(inner, param.as_mut_ptr()) != 0 {
                    return None;
                }

                let mut param = param.assume_init();
                param.iPicWidth = 1920;
                param.iPicHeight = 1080;
                param.iTargetBitrate = 1_000_000;
                param.fMaxFrameRate = 30.0;
                param.iUsageType = sys::EUsageType_CAMERA_VIDEO_REAL_TIME;

                // 空間レイヤーにプロファイルを設定する
                for layer in &mut param.sSpatialLayers[..param.iSpatialLayerNum as usize] {
                    layer.uiProfileIdc = profile_idc;
                    layer.iVideoWidth = 1920;
                    layer.iVideoHeight = 1080;
                    layer.fFrameRate = 30.0;
                    layer.iSpatialBitrate = 1_000_000;
                    layer.iMaxSpatialBitrate = 2_000_000;
                }

                let init = (**inner).InitializeExt?;
                Some(init(inner, &param) == 0)
            })();

            if let Some(uninit) = (**inner).Uninitialize {
                uninit(inner);
            }
            let _ = self.call("WelsDestroySVCEncoder", |f: WelsDestroySVCEncoder| f(inner));

            result.unwrap_or(false)
        }
    }

    fn call<F, T, U>(&self, symbol: &str, f: F) -> Result<U, Error>
    where
        F: FnOnce(T) -> U,
    {
        let func: T = unsafe {
            self.lib
                .get(symbol.as_bytes())
                .map_err(Error::SharedLibraryError)?
        };
        Ok(f(func))
    }
}

// 以下は共有ライブラリからの取得されるそれぞれの関数の型定義。
// Rust では関数から直接その型をコンパイル時に取得する方法がないので、自前で定義している。
type WelsCreateSVCEncoder = unsafe extern "C" fn(pp_encoder: *mut *mut sys::ISVCEncoder) -> c_int;
type WelsDestroySVCEncoder = unsafe extern "C" fn(p_encoder: *mut sys::ISVCEncoder);
type WelsCreateDecoder = unsafe extern "C" fn(pp_decoder: *mut *mut sys::ISVCDecoder) -> c_int;
type WelsDestroyDecoder = unsafe extern "C" fn(p_decoder: *mut sys::ISVCDecoder);

/// H.264 デコーダー
#[derive(Debug)]
pub struct Decoder {
    lib: Openh264Library,
    inner: *mut sys::ISVCDecoder,
}

impl Decoder {
    /// デコーダーインスタンスを生成する
    pub fn new(lib: Openh264Library) -> Result<Self, Error> {
        let mut inner = std::ptr::null_mut();
        let param = MaybeUninit::<sys::SDecodingParam>::zeroed();
        unsafe {
            let name = "WelsCreateDecoder";
            let code = lib.call(name, |f: WelsCreateDecoder| f(&mut inner))?;
            Error::check(code, name)?;

            let mut param = param.assume_init();
            param.pFileNameRestructed = std::ptr::null_mut();
            param.uiTargetDqLayer = 1;
            param.eEcActiveIdc = sys::ERROR_CON_IDC_ERROR_CON_DISABLE;
            param.bParseOnly = false;
            param.sVideoProperty.eVideoBsType = sys::VIDEO_BITSTREAM_TYPE_VIDEO_BITSTREAM_AVC;

            let name = "ISVCDecoder.Initialize";
            let code = (**inner).Initialize.ok_or(Error::UnavailableMethod(name))?(inner, &param);
            Error::check(code as c_int, name)?;

            Ok(Self { lib, inner })
        }
    }

    /// Annex.B 形式の H.264 データをデコードする
    ///
    /// 出力は I420 (YUV 4:2:0 planar) 形式。OpenH264 のデコーダーは I420 のみ出力する。
    /// B フレームは存在しない前提（入力と出力の順番が一致する）。
    pub fn decode(&mut self, data: &[u8]) -> Result<Option<DecodedFrame>, Error> {
        let data_len = c_int::try_from(data.len()).map_err(|_| {
            Error::InvalidParameter("decode input data exceeds c_int::MAX".to_string())
        })?;

        let mut info = MaybeUninit::<sys::SBufferInfo>::zeroed();
        unsafe {
            let mut yuv = [std::ptr::null_mut(); 3];
            let name = "ISVCDecoder.DecodeFrameNoDelay";
            let code = (**self.inner)
                .DecodeFrameNoDelay
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                data.as_ptr(),
                data_len,
                yuv.as_mut_ptr(),
                info.as_mut_ptr(),
            );
            Error::check(code as c_int, name)?;

            let info = info.assume_init();
            if info.iBufferStatus != 1 {
                // ステータスが 1 以外ならまだデコード結果は存在しない。
                // B フレームを扱っていない場合でも、そもそも `data` に映像フレームを含まない NAL ユニットを
                // 指定することはできるので、ここに来る可能性はある。
                return Ok(None);
            }

            if info.UsrData.sSystemBuffer.iFormat != sys::EVideoFormatType_videoFormatI420 as c_int
            {
                // I420 以外は想定外
                return Err(Error::UnsupportedFormat {
                    format: info.UsrData.sSystemBuffer.iFormat as sys::EVideoFormatType,
                });
            }

            Ok(Some(DecodedFrame::from_buffer_info(&info)))
        }
    }

    /// これ以上データが来ないことをデコーダーに伝えて残りの結果を取得する
    pub fn finish(&mut self) -> Result<Option<DecodedFrame>, Error> {
        let mut info = MaybeUninit::<sys::SBufferInfo>::zeroed();
        unsafe {
            let mut yuv = [std::ptr::null_mut(); 3];
            let name = "ISVCDecoder.FlushFrame";
            let code = (**self.inner)
                .FlushFrame
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                yuv.as_mut_ptr(),
                info.as_mut_ptr(),
            );
            Error::check(code as c_int, name)?;

            let info = info.assume_init();
            if info.iBufferStatus != 1 {
                // ステータスが 1 以外ならデコード結果は存在しない。
                return Ok(None);
            }

            if info.UsrData.sSystemBuffer.iFormat != sys::EVideoFormatType_videoFormatI420 as c_int
            {
                // I420 以外は想定外
                return Err(Error::UnsupportedFormat {
                    format: info.UsrData.sSystemBuffer.iFormat as sys::EVideoFormatType,
                });
            }

            Ok(Some(DecodedFrame::from_buffer_info(&info)))
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            if let Some(uninitialize) = (**self.inner).Uninitialize {
                uninitialize(self.inner);
            }
            let _ = self
                .lib
                .call("WelsDestroyDecoder", |f: WelsDestroyDecoder| f(self.inner));
        }
    }
}

unsafe impl Send for Decoder {}

/// デコードされた映像フレーム (I420 / YUV 4:2:0 planar 形式)
///
/// YUV データを所有しているため、デコーダーのライフタイムに依存しない。
#[derive(Debug, Clone)]
pub struct DecodedFrame {
    width: usize,
    height: usize,
    y_stride: usize,
    u_stride: usize,
    v_stride: usize,
    y_data: Vec<u8>,
    u_data: Vec<u8>,
    v_data: Vec<u8>,
}

impl DecodedFrame {
    /// OpenH264 の `SBufferInfo` から YUV データをコピーして `DecodedFrame` を構築する
    unsafe fn from_buffer_info(info: &sys::SBufferInfo) -> Self {
        unsafe {
            let width = info.UsrData.sSystemBuffer.iWidth as usize;
            let height = info.UsrData.sSystemBuffer.iHeight as usize;
            let y_stride = info.UsrData.sSystemBuffer.iStride[0] as usize;
            let u_stride = info.UsrData.sSystemBuffer.iStride[1] as usize;
            let v_stride = u_stride;

            let y_size = height * y_stride;
            let uv_height = height.div_ceil(2);
            let u_size = uv_height * u_stride;
            let v_size = uv_height * v_stride;

            let y_data = std::slice::from_raw_parts(info.pDst[0], y_size).to_vec();
            let u_data = std::slice::from_raw_parts(info.pDst[1], u_size).to_vec();
            let v_data = std::slice::from_raw_parts(info.pDst[2], v_size).to_vec();

            Self {
                width,
                height,
                y_stride,
                u_stride,
                v_stride,
                y_data,
                u_data,
                v_data,
            }
        }
    }

    /// フレームの Y 成分のデータを返す
    pub fn y_plane(&self) -> &[u8] {
        &self.y_data
    }

    /// フレームの U 成分のデータを返す
    pub fn u_plane(&self) -> &[u8] {
        &self.u_data
    }

    /// フレームの V 成分のデータを返す
    pub fn v_plane(&self) -> &[u8] {
        &self.v_data
    }

    /// フレームの Y 成分のストライドを返す
    pub fn y_stride(&self) -> usize {
        self.y_stride
    }

    /// フレームの U 成分のストライドを返す
    pub fn u_stride(&self) -> usize {
        self.u_stride
    }

    /// フレームの V 成分のストライドを返す
    pub fn v_stride(&self) -> usize {
        self.v_stride
    }

    /// フレームの幅を返す
    pub fn width(&self) -> usize {
        self.width
    }

    /// フレームの高さを返す
    pub fn height(&self) -> usize {
        self.height
    }
}

/// エンコーダーに指定する設定
///
/// 必須フィールド以外は `None` で OpenH264 の `GetDefaultParams` の値がそのまま使われる。
#[derive(Debug, Clone)]
pub struct EncoderConfig {
    /// 入出力画像の幅
    pub width: usize,

    /// 入出力画像の高さ
    pub height: usize,

    /// エンコードビットレート (bps 単位)
    pub target_bitrate: usize,

    /// FPS の分子
    pub fps_numerator: usize,

    /// FPS の分母
    pub fps_denominator: usize,

    /// H.264 レベル (None: OpenH264 自動検出)
    pub level: Option<Level>,

    /// エントロピー符号化モード (None: CAVLC)
    ///
    /// CABAC を指定した場合、OpenH264 は SPS の profile_idc を自動的に
    /// PRO_HIGH (100) に設定する。ただし実際の符号化能力は
    /// Constrained Baseline + CABAC に留まる。
    pub entropy_coding_mode: Option<EntropyCodingMode>,

    /// 複雑度モード (None: LOW_COMPLEXITY)
    pub complexity_mode: Option<ComplexityMode>,

    /// 参照フレーム数 (None: OpenH264 自動選択)
    pub ref_frame_count: Option<NonZeroUsize>,

    /// マルチスレッド数 (None: OpenH264 デフォルト)
    pub thread_count: Option<NonZeroUsize>,

    /// 空間レイヤー数 (None: 1)
    pub spatial_layers: Option<NonZeroUsize>,

    /// 時間レイヤー数 (None: 1)
    pub temporal_layers: Option<NonZeroUsize>,

    /// Intra フレーム間隔 (None: OpenH264 デフォルト)
    pub intra_period: Option<usize>,

    /// レート制御モード (None: RC_QUALITY_MODE)
    pub rate_control_mode: Option<RateControlMode>,

    /// 最大 QP 値 (None: 51)
    pub max_qp: Option<usize>,

    /// 最小 QP 値 (None: 0)
    pub min_qp: Option<usize>,

    /// ノイズ除去機能 (None: false)
    pub denoise: Option<bool>,

    /// 背景検出機能 (None: true)
    pub background_detection: Option<bool>,

    /// 適応量子化機能 (None: true)
    pub adaptive_quantization: Option<bool>,

    /// シーン変化検出機能 (None: true)
    pub scene_change_detection: Option<bool>,

    /// デブロッキングフィルタ (None: true)
    pub deblocking_filter: Option<bool>,

    /// 長期参照フレーム機能 (None: false)
    pub long_term_reference: Option<bool>,

    /// スライスモード (None: SM_SINGLE_SLICE)
    pub slice_mode: Option<SliceMode>,
}

impl EncoderConfig {
    /// 必須フィールドのみを指定して `EncoderConfig` を生成する
    ///
    /// オプションフィールドはすべて `None` (OpenH264 のデフォルト値) になる。
    pub fn new(
        width: usize,
        height: usize,
        target_bitrate: usize,
        fps_numerator: usize,
        fps_denominator: usize,
    ) -> Self {
        Self {
            width,
            height,
            target_bitrate,
            fps_numerator,
            fps_denominator,
            level: None,
            entropy_coding_mode: None,
            complexity_mode: None,
            ref_frame_count: None,
            thread_count: None,
            spatial_layers: None,
            temporal_layers: None,
            intra_period: None,
            rate_control_mode: None,
            max_qp: None,
            min_qp: None,
            denoise: None,
            background_detection: None,
            adaptive_quantization: None,
            scene_change_detection: None,
            deblocking_filter: None,
            long_term_reference: None,
            slice_mode: None,
        }
    }
}

/// 複雑度モード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityMode {
    /// 最低複雑度 (最高速)
    Low,
    /// 中程度複雑度
    Medium,
    /// 高複雑度 (最高品質)
    High,
}

/// レート制御モード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateControlMode {
    /// レート制御無効 (最高速)
    Off,
    /// 品質モード
    Quality,
    /// ビットレートモード
    Bitrate,
    /// タイムスタンプモード
    Timestamp,
}

/// スライスモード
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SliceMode {
    /// 単一スライス (最高速)
    Single,
    /// 固定スライス数
    FixedCount(usize),
    /// サイズ制限スライス
    SizeConstrained(usize),
}

impl Level {
    fn to_sys(self) -> sys::ELevelIdc {
        match self {
            Level::L1 => sys::ELevelIdc_LEVEL_1_0,
            Level::L1_1 => sys::ELevelIdc_LEVEL_1_1,
            Level::L1_2 => sys::ELevelIdc_LEVEL_1_2,
            Level::L1_3 => sys::ELevelIdc_LEVEL_1_3,
            Level::L2 => sys::ELevelIdc_LEVEL_2_0,
            Level::L2_1 => sys::ELevelIdc_LEVEL_2_1,
            Level::L2_2 => sys::ELevelIdc_LEVEL_2_2,
            Level::L3 => sys::ELevelIdc_LEVEL_3_0,
            Level::L3_1 => sys::ELevelIdc_LEVEL_3_1,
            Level::L3_2 => sys::ELevelIdc_LEVEL_3_2,
            Level::L4 => sys::ELevelIdc_LEVEL_4_0,
            Level::L4_1 => sys::ELevelIdc_LEVEL_4_1,
            Level::L4_2 => sys::ELevelIdc_LEVEL_4_2,
            Level::L5 => sys::ELevelIdc_LEVEL_5_0,
            Level::L5_1 => sys::ELevelIdc_LEVEL_5_1,
            Level::L5_2 => sys::ELevelIdc_LEVEL_5_2,
        }
    }
}

/// H.264 エンコーダー
#[derive(Debug)]
pub struct Encoder {
    lib: Openh264Library,
    inner: *mut sys::ISVCEncoder,
    pic: sys::SSourcePicture,
    frames: usize,
    fps_numerator: usize,
    fps_denominator: usize,
}

/// `EncoderConfig` の内容を `SEncParamExt` に反映する
///
/// `new()` と `reconfigure()` の共通処理。
fn apply_config_to_param(param: &mut sys::SEncParamExt, config: &EncoderConfig) {
    param.iUsageType = sys::EUsageType_CAMERA_VIDEO_REAL_TIME;
    param.fMaxFrameRate = config.fps_numerator as f32 / config.fps_denominator as f32;
    param.iPicWidth = config.width as c_int;
    param.iPicHeight = config.height as c_int;
    param.iTargetBitrate = config.target_bitrate as c_int;

    // 以下は None なら既存値 (GetDefaultParams の値) をそのまま使う

    if let Some(mode) = config.complexity_mode {
        param.iComplexityMode = match mode {
            ComplexityMode::Low => sys::ECOMPLEXITY_MODE_LOW_COMPLEXITY,
            ComplexityMode::Medium => sys::ECOMPLEXITY_MODE_MEDIUM_COMPLEXITY,
            ComplexityMode::High => sys::ECOMPLEXITY_MODE_HIGH_COMPLEXITY,
        };
    }

    if let Some(mode) = config.entropy_coding_mode {
        param.iEntropyCodingModeFlag = match mode {
            EntropyCodingMode::Cavlc => 0,
            EntropyCodingMode::Cabac => 1,
        };
    }

    if let Some(count) = config.ref_frame_count {
        param.iNumRefFrame = count.get() as c_int;
    }

    if let Some(count) = config.thread_count {
        param.iMultipleThreadIdc = count.get() as c_ushort;
    }

    if let Some(layers) = config.spatial_layers {
        param.iSpatialLayerNum = layers.get() as c_int;
    }

    if let Some(layers) = config.temporal_layers {
        param.iTemporalLayerNum = layers.get() as c_int;
    }

    if let Some(intra) = config.intra_period {
        param.uiIntraPeriod = intra as c_uint;
    }

    if let Some(mode) = config.rate_control_mode {
        param.iRCMode = match mode {
            RateControlMode::Off => sys::RC_MODES_RC_OFF_MODE,
            RateControlMode::Quality => sys::RC_MODES_RC_QUALITY_MODE,
            RateControlMode::Bitrate => sys::RC_MODES_RC_BITRATE_MODE,
            RateControlMode::Timestamp => sys::RC_MODES_RC_TIMESTAMP_MODE,
        };
    }

    if let Some(qp) = config.max_qp {
        param.iMaxQp = qp as i32;
    }

    if let Some(qp) = config.min_qp {
        param.iMinQp = qp as i32;
    }

    if let Some(v) = config.denoise {
        param.bEnableDenoise = v;
    }

    if let Some(v) = config.background_detection {
        param.bEnableBackgroundDetection = v;
    }

    if let Some(v) = config.adaptive_quantization {
        param.bEnableAdaptiveQuant = v;
    }

    if let Some(v) = config.scene_change_detection {
        param.bEnableSceneChangeDetect = v;
    }

    if let Some(v) = config.deblocking_filter {
        param.iLoopFilterDisableIdc = if v { 0 } else { 1 };
    }

    if let Some(v) = config.long_term_reference {
        if v {
            param.bEnableLongTermReference = true;
            param.iLTRRefNum = 1;
        } else {
            param.bEnableLongTermReference = false;
            param.iLTRRefNum = 0;
        }
    }

    // 空間レイヤー設定
    // OpenH264 は Constrained Baseline Profile のみ対応 (README 参照)。
    // プロファイルは PRO_UNKNOWN のまま残し、entropy_coding_mode に基づいて
    // OpenH264 が自動選択する (CAVLC → PRO_BASELINE, CABAC → PRO_HIGH)。
    for layer in &mut param.sSpatialLayers[..param.iSpatialLayerNum as usize] {
        if let Some(level) = config.level {
            layer.uiLevelIdc = level.to_sys();
        }
        layer.iVideoWidth = config.width as c_int;
        layer.iVideoHeight = config.height as c_int;
        layer.fFrameRate = param.fMaxFrameRate;
        layer.iSpatialBitrate = config.target_bitrate as c_int;
        layer.iMaxSpatialBitrate = (config.target_bitrate * 2) as c_int;

        if let Some(slice_mode) = config.slice_mode {
            match slice_mode {
                SliceMode::Single => {
                    layer.sSliceArgument.uiSliceMode = sys::SliceModeEnum_SM_SINGLE_SLICE;
                }
                SliceMode::FixedCount(count) => {
                    layer.sSliceArgument.uiSliceMode = sys::SliceModeEnum_SM_FIXEDSLCNUM_SLICE;
                    layer.sSliceArgument.uiSliceNum = count as c_uint;
                }
                SliceMode::SizeConstrained(size) => {
                    layer.sSliceArgument.uiSliceMode = sys::SliceModeEnum_SM_SIZELIMITED_SLICE;
                    layer.sSliceArgument.uiSliceSizeConstraint = size as c_uint;
                }
            }
        }
    }
}

impl Encoder {
    /// エンコーダーインスタンスを生成する
    pub fn new(lib: Openh264Library, config: EncoderConfig) -> Result<Self, Error> {
        if config.fps_numerator == 0 || config.fps_denominator == 0 {
            return Err(Error::InvalidParameter(
                "fps_numerator and fps_denominator must be non-zero".to_string(),
            ));
        }

        let mut inner = std::ptr::null_mut();
        let mut param = MaybeUninit::<sys::SEncParamExt>::zeroed();
        let pic = MaybeUninit::<sys::SSourcePicture>::zeroed();
        unsafe {
            let name = "WelsCreateSVCEncoder";
            let code = lib.call(name, |f: WelsCreateSVCEncoder| f(&mut inner))?;
            Error::check(code, name)?;

            let name = "ISVCEncoder.GetDefaultParams";
            let code = (**inner)
                .GetDefaultParams
                .ok_or(Error::UnavailableMethod(name))?(
                inner, param.as_mut_ptr()
            );
            Error::check(code, name)?;

            let mut param = param.assume_init();
            apply_config_to_param(&mut param, &config);

            let name = "ISVCEncoder.InitializeExt";
            let code = (**inner)
                .InitializeExt
                .ok_or(Error::UnavailableMethod(name))?(inner, &param);
            Error::check(code, name)?;

            // I420 フォーマット設定
            let mut i420 = sys::EVideoFormatType_videoFormatI420;
            let name = "ISVCEncoder.SetOption";
            let code = (**inner).SetOption.ok_or(Error::UnavailableMethod(name))?(
                inner,
                sys::ENCODER_OPTION_ENCODER_OPTION_DATAFORMAT,
                std::ptr::from_mut(&mut i420).cast(),
            );
            Error::check(code, name)?;

            // 画像設定
            let mut pic = pic.assume_init();
            pic.iPicWidth = config.width as c_int;
            pic.iPicHeight = config.height as c_int;
            pic.iColorFormat = i420 as c_int;
            pic.iStride[0] = pic.iPicWidth;
            pic.iStride[1] = config.width.div_ceil(2) as c_int;
            pic.iStride[2] = config.width.div_ceil(2) as c_int;

            Ok(Self {
                lib,
                inner,
                pic,
                frames: 0,
                fps_numerator: config.fps_numerator,
                fps_denominator: config.fps_denominator,
            })
        }
    }

    /// ビットレートを動的に変更する
    ///
    /// OpenH264 の `SetOption(ENCODER_OPTION_BITRATE)` を使用する。
    /// 全空間レイヤーに対して一括で適用される。
    pub fn set_bitrate(&mut self, bitrate: usize) -> Result<(), Error> {
        unsafe {
            let mut info = sys::SBitrateInfo {
                iLayer: sys::LAYER_NUM_SPATIAL_LAYER_ALL,
                iBitrate: bitrate as c_int,
            };
            let name = "ISVCEncoder.SetOption";
            let code = (**self.inner)
                .SetOption
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                sys::ENCODER_OPTION_ENCODER_OPTION_BITRATE,
                std::ptr::from_mut(&mut info).cast(),
            );
            Error::check(code as c_int, name)?;
            Ok(())
        }
    }

    /// フレームレートを動的に変更する
    ///
    /// OpenH264 の `SetOption(ENCODER_OPTION_FRAME_RATE)` を使用する。
    pub fn set_frame_rate(
        &mut self,
        fps_numerator: usize,
        fps_denominator: usize,
    ) -> Result<(), Error> {
        if fps_numerator == 0 || fps_denominator == 0 {
            return Err(Error::InvalidParameter(
                "fps_numerator and fps_denominator must be non-zero".to_string(),
            ));
        }

        unsafe {
            let mut fps = fps_numerator as f32 / fps_denominator as f32;
            let name = "ISVCEncoder.SetOption";
            let code = (**self.inner)
                .SetOption
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                sys::ENCODER_OPTION_ENCODER_OPTION_FRAME_RATE,
                std::ptr::from_mut(&mut fps).cast(),
            );
            Error::check(code as c_int, name)?;

            self.fps_numerator = fps_numerator;
            self.fps_denominator = fps_denominator;

            Ok(())
        }
    }

    /// 解像度を動的に変更する
    ///
    /// OpenH264 の `SetOption(ENCODER_OPTION_SVC_ENCODE_PARAM_EXT)` で
    /// 現在のパラメーターを維持したまま解像度のみ変更する。
    pub fn set_resolution(&mut self, width: usize, height: usize) -> Result<(), Error> {
        unsafe {
            let mut param = MaybeUninit::<sys::SEncParamExt>::zeroed();
            let name = "ISVCEncoder.GetOption";
            let code = (**self.inner)
                .GetOption
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                sys::ENCODER_OPTION_ENCODER_OPTION_SVC_ENCODE_PARAM_EXT,
                param.as_mut_ptr().cast(),
            );
            Error::check(code as c_int, name)?;

            let mut param = param.assume_init();
            param.iPicWidth = width as c_int;
            param.iPicHeight = height as c_int;

            for layer in &mut param.sSpatialLayers[..param.iSpatialLayerNum as usize] {
                layer.iVideoWidth = width as c_int;
                layer.iVideoHeight = height as c_int;
            }

            let name = "ISVCEncoder.SetOption";
            let code = (**self.inner)
                .SetOption
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                sys::ENCODER_OPTION_ENCODER_OPTION_SVC_ENCODE_PARAM_EXT,
                std::ptr::from_mut(&mut param).cast(),
            );
            Error::check(code as c_int, name)?;

            self.pic.iPicWidth = width as c_int;
            self.pic.iPicHeight = height as c_int;
            self.pic.iStride[0] = width as c_int;
            self.pic.iStride[1] = width.div_ceil(2) as c_int;
            self.pic.iStride[2] = width.div_ceil(2) as c_int;

            Ok(())
        }
    }

    /// エンコーダーの全パラメーターを動的に変更する
    ///
    /// 解像度、ビットレート、フレームレート等を含む全パラメーターを再設定する。
    /// OpenH264 の `SetOption(ENCODER_OPTION_SVC_ENCODE_PARAM_EXT)` を使用するため、
    /// エンコーダーの再生成よりも軽量。
    pub fn set_config(&mut self, config: EncoderConfig) -> Result<(), Error> {
        if config.fps_numerator == 0 || config.fps_denominator == 0 {
            return Err(Error::InvalidParameter(
                "fps_numerator and fps_denominator must be non-zero".to_string(),
            ));
        }

        unsafe {
            let mut param = MaybeUninit::<sys::SEncParamExt>::zeroed();
            let name = "ISVCEncoder.GetOption";
            let code = (**self.inner)
                .GetOption
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                sys::ENCODER_OPTION_ENCODER_OPTION_SVC_ENCODE_PARAM_EXT,
                param.as_mut_ptr().cast(),
            );
            Error::check(code as c_int, name)?;

            let mut param = param.assume_init();
            apply_config_to_param(&mut param, &config);

            let name = "ISVCEncoder.SetOption";
            let code = (**self.inner)
                .SetOption
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                sys::ENCODER_OPTION_ENCODER_OPTION_SVC_ENCODE_PARAM_EXT,
                std::ptr::from_mut(&mut param).cast(),
            );
            Error::check(code as c_int, name)?;

            self.pic.iPicWidth = config.width as c_int;
            self.pic.iPicHeight = config.height as c_int;
            self.pic.iStride[0] = config.width as c_int;
            self.pic.iStride[1] = config.width.div_ceil(2) as c_int;
            self.pic.iStride[2] = config.width.div_ceil(2) as c_int;

            self.fps_numerator = config.fps_numerator;
            self.fps_denominator = config.fps_denominator;

            Ok(())
        }
    }

    /// I420 (YUV 4:2:0 planar) 形式の画像データをエンコードする
    ///
    /// OpenH264 のエンコーダーは I420 のみ受け付ける。
    /// `y` のストライドは入力フレームの幅と等しいことが前提。
    /// B フレームは扱わない前提（入力フレームと出力フレームの順番が一致する）。
    pub fn encode(
        &mut self,
        y: &[u8],
        u: &[u8],
        v: &[u8],
        options: &EncodeOptions,
    ) -> Result<Option<EncodedFrame>, Error> {
        let height = self.pic.iPicHeight as usize;
        let y_size = height * self.pic.iStride[0] as usize;
        let u_size = height.div_ceil(2) * self.pic.iStride[1] as usize;
        let v_size = u_size;
        if y.len() != y_size || u.len() != u_size || v.len() != v_size {
            return Err(Error::InvalidYuvSize);
        }

        unsafe {
            // ForceIntraFrame 処理
            if options.force_idr {
                let name = "ISVCEncoder.ForceIntraFrame";
                let code = (**self.inner)
                    .ForceIntraFrame
                    .ok_or(Error::UnavailableMethod(name))?(
                    self.inner, true
                );
                Error::check(code, name)?;
            }

            self.pic.pData[0] = y.as_ptr().cast_mut();
            self.pic.pData[1] = u.as_ptr().cast_mut();
            self.pic.pData[2] = v.as_ptr().cast_mut();

            let timestamp = Duration::from_secs((self.frames * self.fps_denominator) as u64)
                / self.fps_numerator as u32;
            self.pic.uiTimeStamp = timestamp.as_millis() as c_longlong; // openh264 はミリ秒固定
            self.frames += 1;

            let mut info = MaybeUninit::<sys::SFrameBSInfo>::zeroed();
            let name = "ISVCEncoder.EncodeFrame";
            let code = (**self.inner)
                .EncodeFrame
                .ok_or(Error::UnavailableMethod(name))?(
                self.inner,
                &mut self.pic,
                info.as_mut_ptr(),
            );
            Error::check(code, name)?;

            let info = info.assume_init();
            if info.eFrameType == sys::EVideoFrameType_videoFrameTypeSkip {
                return Ok(None);
            }

            let frame_type = match info.eFrameType {
                sys::EVideoFrameType_videoFrameTypeIDR => FrameType::Idr,
                sys::EVideoFrameType_videoFrameTypeI => FrameType::I,
                _ => FrameType::P,
            };

            // SPS/PPS を分離して EncodedFrame を構築
            let mut sps_list = Vec::new();
            let mut pps_list = Vec::new();
            let mut data = Vec::new();

            for layer_info in &info.sLayerInfo[..info.iLayerNum as usize] {
                if layer_info.iNalCount == 0 {
                    // カウントがゼロの場合には、環境によっては、
                    // pNalLengthInByte が不正なアドレスを指していて from_raw_parts() がクラッシュする
                    // 可能性があるので明示的にハンドリングする
                    continue;
                }

                let nal_lengths = std::slice::from_raw_parts(
                    layer_info.pNalLengthInByte,
                    layer_info.iNalCount as usize,
                );

                let mut offset = 0usize;
                for &nal_len in nal_lengths {
                    let nal_len = nal_len as usize;
                    let nal_data =
                        std::slice::from_raw_parts(layer_info.pBsBuf.add(offset), nal_len);

                    // Annex B スタートコードをスキップして NALU 本体を取得
                    let nalu_body = skip_start_code(nal_data);

                    if !nalu_body.is_empty() {
                        let nal_type = nalu_body[0] & 0x1F;
                        match nal_type {
                            7 => {
                                // SPS: スタートコードを除いた NALU 本体を保存
                                sps_list.push(nalu_body.to_vec());
                            }
                            8 => {
                                // PPS: スタートコードを除いた NALU 本体を保存
                                pps_list.push(nalu_body.to_vec());
                            }
                            _ => {
                                // その他の NALU: Annex B 形式のまま data に追加
                                data.extend_from_slice(nal_data);
                            }
                        }
                    } else {
                        // パースできない場合はそのまま data に追加
                        data.extend_from_slice(nal_data);
                    }

                    offset += nal_len;
                }
            }

            Ok(Some(EncodedFrame {
                frame_type,
                sps_list,
                pps_list,
                data,
            }))
        }
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            if let Some(uninitialize) = (**self.inner).Uninitialize {
                uninitialize(self.inner);
            }
            let _ = self
                .lib
                .call("WelsDestroySVCEncoder", |f: WelsDestroySVCEncoder| {
                    f(self.inner)
                });
        }
    }
}

unsafe impl Send for Encoder {}

/// Annex B スタートコード (0x00000001 または 0x000001) をスキップして NALU 本体を返す
fn skip_start_code(data: &[u8]) -> &[u8] {
    if data.len() >= 4 && data[0] == 0 && data[1] == 0 && data[2] == 0 && data[3] == 1 {
        &data[4..]
    } else if data.len() >= 3 && data[0] == 0 && data[1] == 0 && data[2] == 1 {
        &data[3..]
    } else {
        data
    }
}

/// エンコードされた映像フレーム
#[derive(Debug, Clone)]
pub struct EncodedFrame {
    /// フレームタイプ
    pub frame_type: FrameType,

    /// SPS (IDR フレーム時のみ含まれる)
    pub sps_list: Vec<Vec<u8>>,

    /// PPS (IDR フレーム時のみ含まれる)
    pub pps_list: Vec<Vec<u8>>,

    /// 圧縮データ (Annex B 形式、SPS/PPS の NALU を除外)
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_library() {
        let Ok(path) = std::env::var("OPENH264_PATH") else {
            panic!("OPENH264_PATH env var is not found");
        };

        assert!(Openh264Library::load(path).is_ok());
    }

    #[test]
    fn init_decoder() {
        let Ok(path) = std::env::var("OPENH264_PATH") else {
            panic!("OPENH264_PATH env var is not found");
        };

        let lib = Openh264Library::load(path).expect("load library error");
        assert!(Decoder::new(lib).is_ok());
    }

    #[test]
    fn decode_black() {
        let Ok(path) = std::env::var("OPENH264_PATH") else {
            panic!("OPENH264_PATH env var is not found");
        };

        let lib = Openh264Library::load(path).expect("load library error");
        let mut decoder = Decoder::new(lib).expect("create decoder error");
        let mut decoded_count = 0;

        let data = [
            // SPS
            0, 0, 0, 1, 103, 100, 0, 30, 172, 217, 64, 160, 61, 176, 17, 0, 0, 3, 0, 1, 0, 0, 3, 0,
            50, 15, 22, 45, 150, //
            // PPS
            0, 0, 0, 1, 104, 235, 227, 203, 34, 192, //
            // 映像データ
            0, 0, 0, 1, 101, 136, 132, 0, 43, 255, 254, 246, 115, 124, 10, 107, 109, 176, 149, 46,
            5, 118, 247, 102, 163, 229, 208, 146, 229, 251, 16, 96, 250, 208, 0, 0, 3, 0, 0, 3, 0,
            0, 16, 15, 210, 222, 245, 204, 98, 91, 229, 32, 0, 0, 9, 216, 2, 56, 13, 16, 118, 133,
            116, 69, 196, 32, 71, 6, 120, 150, 16, 161, 210, 50, 128, 0, 0, 3, 0, 0, 3, 0, 0, 3, 0,
            0, 3, 0, 0, 3, 0, 0, 3, 0, 0, 3, 0, 0, 3, 0, 0, 3, 0, 37, 225,
        ];
        decoded_count += decoder.decode(&data).expect("decode error").is_some() as usize;
        decoded_count += decoder.finish().expect("decode error").is_some() as usize;
        assert_eq!(decoded_count, 1);
    }

    #[test]
    fn supported_codecs() {
        let Ok(path) = std::env::var("OPENH264_PATH") else {
            panic!("OPENH264_PATH env var is not found");
        };

        let lib = Openh264Library::load(path).expect("load library error");
        let info = lib.supported_codecs();

        assert_eq!(info.codec, VideoCodecType::H264);

        // デコード対応
        assert!(info.decoding.supported);
        assert!(!info.decoding.hardware_accelerated);

        // エンコード対応
        assert!(info.encoding.supported);
        assert!(!info.encoding.hardware_accelerated);

        // OpenH264 は Constrained Baseline Profile のみ対応
        match &info.encoding.profiles {
            EncodingProfiles::H264(profiles) => {
                assert_eq!(profiles.len(), 1);
                assert_eq!(profiles[0], H264EncodingProfile::ConstrainedBaseline);
            }
            EncodingProfiles::Unsupported => {
                panic!("encoding profiles should not be Unsupported");
            }
        }
    }

    #[test]
    fn init_encoder() {
        let Ok(path) = std::env::var("OPENH264_PATH") else {
            panic!("OPENH264_PATH env var is not found");
        };

        let lib = Openh264Library::load(path).expect("load library error");
        let config = EncoderConfig::new(64, 64, 100_000, 1, 1);
        assert!(Encoder::new(lib, config).is_ok());
    }

    #[test]
    fn encode_black() {
        let Ok(path) = std::env::var("OPENH264_PATH") else {
            panic!("OPENH264_PATH env var is not found");
        };

        let lib = Openh264Library::load(path).expect("load library error");
        let config = EncoderConfig::new(64, 64, 100_000, 1, 1);
        let mut encoder = Encoder::new(lib, config).expect("create encoder error");
        let encoded = encoder
            .encode(
                &[0; 64 * 64],
                &[0; 32 * 32],
                &[0; 32 * 32],
                &EncodeOptions::default(),
            )
            .expect("encode error");
        assert!(encoded.is_some());
        let encoded = encoded.unwrap();
        assert_eq!(encoded.frame_type, FrameType::Idr);
        assert!(!encoded.sps_list.is_empty());
        assert!(!encoded.pps_list.is_empty());
        assert!(!encoded.data.is_empty());
    }
}

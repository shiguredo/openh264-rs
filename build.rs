//! openh264 のソースコードを git clone し、ヘッダーファイル (codec_api.h) から
//! bindgen で FFI バインディングを生成するビルドスクリプト。
//! openh264 自体の C/C++ コンパイルやリンクは行わない。

use std::{
    path::{Path, PathBuf},
    process::Command,
};

// 依存ライブラリの名前
const LIB_NAME: &str = "openh264";

fn main() {
    // Cargo.toml か build.rs が更新されたら、依存ライブラリを再ビルドする
    println!("cargo::rerun-if-changed=Cargo.toml");
    println!("cargo::rerun-if-changed=build.rs");

    // 各種変数やビルドディレクトリのセットアップ
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("infallible"));
    let out_build_dir = out_dir.join("build");
    let src_dir = out_build_dir.join(LIB_NAME);
    let input_header_path = src_dir.join("codec/api/wels/codec_api.h");
    let output_metadata_path = out_dir.join("metadata.rs");
    let output_bindings_path = out_dir.join("bindings.rs");
    if out_build_dir.exists() {
        std::fs::remove_dir_all(&out_build_dir).expect("failed to remove build directory");
    }
    std::fs::create_dir(&out_build_dir).expect("failed to create build directory");

    // Cargo.toml から依存ライブラリの URL とバージョンタグを取得する
    let (url, version) = get_url_and_version();

    // 各種メタデータを書き込む
    std::fs::write(
        output_metadata_path,
        format!(
            concat!(
                "pub const BUILD_METADATA_REPOSITORY: &str={:?};\n",
                "pub const BUILD_METADATA_VERSION: &str={:?};\n",
            ),
            url, version
        ),
    )
    .expect("failed to write metadata file");

    if std::env::var("DOCS_RS").is_ok() {
        // Docs.rs 向けのビルドでは git clone ができないので build.rs の処理はスキップして、
        // 代わりに、ドキュメント生成時に最低限必要な定義だけをダミーで出力している。
        //
        // See also: https://docs.rs/about/builds
        std::fs::write(
            output_bindings_path,
            concat!(
                "pub type EProfileIdc = u32;\n",
                "pub type ELevelIdc = u32;\n",
                "pub type EVideoFormatType = u32;\n",
                "pub struct SSourcePicture;\n",
                "pub struct SBufferInfo;\n",
                "pub struct ISVCEncoder;\n",
                "pub struct ISVCDecoder;\n",
                "pub struct OpenH264Version;\n",
                "pub const EProfileIdc_PRO_BASELINE: EProfileIdc = 66;\n",
                "pub const ELevelIdc_LEVEL_1_0: ELevelIdc = 10;\n",
                "pub const ELevelIdc_LEVEL_1_1: ELevelIdc = 11;\n",
                "pub const ELevelIdc_LEVEL_1_2: ELevelIdc = 12;\n",
                "pub const ELevelIdc_LEVEL_1_3: ELevelIdc = 13;\n",
                "pub const ELevelIdc_LEVEL_2_0: ELevelIdc = 20;\n",
                "pub const ELevelIdc_LEVEL_2_1: ELevelIdc = 21;\n",
                "pub const ELevelIdc_LEVEL_2_2: ELevelIdc = 22;\n",
                "pub const ELevelIdc_LEVEL_3_0: ELevelIdc = 30;\n",
                "pub const ELevelIdc_LEVEL_3_1: ELevelIdc = 31;\n",
                "pub const ELevelIdc_LEVEL_3_2: ELevelIdc = 32;\n",
                "pub const ELevelIdc_LEVEL_4_0: ELevelIdc = 40;\n",
                "pub const ELevelIdc_LEVEL_4_1: ELevelIdc = 41;\n",
                "pub const ELevelIdc_LEVEL_4_2: ELevelIdc = 42;\n",
                "pub const ELevelIdc_LEVEL_5_0: ELevelIdc = 50;\n",
                "pub const ELevelIdc_LEVEL_5_1: ELevelIdc = 51;\n",
                "pub const ELevelIdc_LEVEL_5_2: ELevelIdc = 52;\n",
            ),
        )
        .expect("failed to write bindings");
        return;
    }

    // 依存ライブラリのリポジトリを取得する
    git_clone_external_lib(&out_build_dir, &url, &version);

    // バインディングを生成する
    bindgen::Builder::default()
        .header(input_header_path.to_str().expect("invalid header path"))
        .generate()
        .expect("failed to generate bindings")
        .write_to_file(output_bindings_path)
        .expect("failed to write bindings");
}

// 外部ライブラリのリポジトリを git clone する
fn git_clone_external_lib(build_dir: &Path, url: &str, version: &str) {
    let status = Command::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg("--quiet")
        .arg("--branch")
        .arg(version)
        .arg(url)
        .current_dir(build_dir)
        .status()
        .expect("failed to execute git");
    if !status.success() {
        panic!("failed to clone {LIB_NAME} repository: {status}");
    }
}

// Cargo.toml から依存ライブラリの URL とバージョンタグを取得する
fn get_url_and_version() -> (String, String) {
    let cargo_toml =
        shiguredo_toml::from_str(include_str!("Cargo.toml")).expect("failed to parse Cargo.toml");
    let deps = cargo_toml
        .get("package")
        .and_then(|v| v.get("metadata"))
        .and_then(|v| v.get("external-dependencies"))
        .and_then(|v| v.get(LIB_NAME))
        .unwrap_or_else(|| {
            panic!(
                "Cargo.toml does not contain [package.metadata.external-dependencies.{LIB_NAME}]"
            )
        });
    let url = deps
        .get("url")
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| panic!("missing 'url' in external-dependencies.{LIB_NAME}"));
    let version = deps
        .get("version")
        .and_then(|s| s.as_str())
        .unwrap_or_else(|| panic!("missing 'version' in external-dependencies.{LIB_NAME}"));
    (url.to_string(), version.to_string())
}

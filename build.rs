//! Build script: downloads the whisper GGML model if not already present.

use std::path::PathBuf;
use std::process::Command;

const MODEL_NAME: &str = "ggml-base.en.bin";
const MODEL_URL: &str =
    "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin";

fn main() {
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());

    // -----------------------------------------------------------------------
    // Whisper model download
    // -----------------------------------------------------------------------
    let model_path = out_dir.join(MODEL_NAME);

    if !model_path.exists() {
        eprintln!("Downloading whisper model to {} ...", model_path.display());

        let status = Command::new("curl")
            .args([
                "-L",
                "--fail",
                "--silent",
                "--show-error",
                "-o",
                model_path.to_str().unwrap(),
                MODEL_URL,
            ])
            .status()
            .expect("failed to run curl — is it installed?");

        assert!(
            status.success(),
            "Failed to download whisper model from {}",
            MODEL_URL
        );

        eprintln!("Whisper model downloaded successfully.");
    }

    println!("cargo:rustc-env=WHISPER_MODEL_PATH={}", model_path.display());

    // Only re-run if relevant files change.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=OUT_DIR");
}

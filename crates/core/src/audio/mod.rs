//! Audio + ML pipeline.
//!
//! v0.1 ships with **stubs only** behind the `audio` feature flag. The
//! `Engine::capture(NewTask)` entry point is the integration seam — the
//! real pipeline will call it the same way the manual-entry path does.
//!
//! Real components, planned:
//!   - cpal mic capture (16 kHz mono)
//!   - Silero VAD (ONNX, ~1 MB)
//!   - ECAPA-TDNN speaker verification (relaxed cosine threshold)
//!   - whisper.cpp `base.en`
//!   - Qwen2.5-3B-Instruct via llama.cpp, GBNF-constrained JSON output
//!
//! All are kept off the default build so the rest of the app can ship and be
//! tested without 3 GB of models. Flip the `audio` feature to plug them in.

#[cfg(not(feature = "audio"))]
mod stub {
    //! No-op pipeline. The Engine still works — manual entry and provider
    //! pushes function end-to-end without any audio hardware.
}

#[cfg(feature = "audio")]
mod real {
    // TODO(v0.2): wire up cpal -> Silero -> ECAPA -> whisper -> qwen -> Engine.capture
}

/// Marker the FFI layer can query so the UI can show "Audio: stubbed" until
/// the real pipeline is enabled.
pub fn is_real() -> bool {
    cfg!(feature = "audio")
}

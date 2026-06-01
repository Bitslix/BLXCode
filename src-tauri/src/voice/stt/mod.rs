//! Speech-to-text backends.
//!
//! - [`cloud`] – OpenAI / OpenRouter audio-transcription HTTP calls (reused by
//!   the existing voice-orb flow and by push-to-talk in cloud mode).
//! - [`local_whisper`] – warm, on-device whisper.cpp via the optional
//!   `local-whisper` feature.
//!
//! Both expose a PCM entry point so the push-to-talk path can hand over an
//! in-memory buffer without ever writing a temp WAV.

pub mod cloud;
pub mod local_whisper;

pub use local_whisper::WhisperEngine;

// Re-exported for the existing voice-orb command path (`stt::transcribe_wav`).
pub use cloud::transcribe_wav;

/// Encode a mono f32 PCM buffer as a 16-bit WAV blob in memory. Shared by the
/// cloud PCM path (multipart upload) and any future in-memory consumer.
pub fn pcm_to_wav_bytes(pcm: &[f32], sample_rate: u32) -> Result<Vec<u8>, String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
    {
        let mut writer =
            hound::WavWriter::new(&mut cursor, spec).map_err(|e| format!("wav writer: {e}"))?;
        for &s in pcm {
            let clamped = s.clamp(-1.0, 1.0);
            let s16 = (clamped * i16::MAX as f32) as i16;
            writer
                .write_sample(s16)
                .map_err(|e| format!("wav write: {e}"))?;
        }
        writer
            .finalize()
            .map_err(|e| format!("wav finalize: {e}"))?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pcm_to_wav_has_riff_header_and_payload() {
        let pcm = vec![0.0_f32, 0.5, -0.5, 1.0, -1.0];
        let wav = pcm_to_wav_bytes(&pcm, 16_000).expect("encode");
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        // 44-byte canonical header + 2 bytes per sample.
        assert_eq!(wav.len(), 44 + pcm.len() * 2);
    }
}

//! 리눅스 접근성 비프 통지.
//!
//! 시각장애 사용자가 화면 없이도 한/영 모드 전환·AutoTypeFix 토글을 소리 높낮이와
//! 횟수로 확인할 수 있게 한다. Windows TSF(`unim-tsf/src/lang_bar.rs`)의 의미론(한글·
//! 켜짐 = 높은 음 2번, 영문·꺼짐 = 낮은 음 1번)과 동일한 청각 피드백을 리눅스 데몬
//! 경로에 제공한다.
//!
//! 신규 크레이트 의존은 0 — 순수 `std` 로 16-bit mono 44.1kHz WAV 를 메모리 합성하고,
//! PATH 에서 탐지한 외부 재생기(`paplay` → `pw-cat` → `aplay`)에 stdin 파이프로 넘긴다.
//! 재생은 별도 스레드에서 좀비를 회수하므로 엔진 워커 스레드는 완전 논블로킹이며,
//! 재생기 부재/실패 시 조용한 no-op 이다(오디오 장치 불필요).

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// 합성 샘플레이트 (Hz).
const SAMPLE_RATE: u32 = 44_100;

/// WAV stdin 파이프를 받는 외부 재생기 명령.
#[derive(Clone, Copy)]
struct Player {
    /// 실행 파일명 (PATH 탐지 대상).
    program: &'static str,
    /// 고정 인자 (stdin 재생 지정 등).
    args: &'static [&'static str],
}

/// 재생기 탐지: `paplay` → `pw-cat --playback -` → `aplay -q -` 순서로 PATH 에서
/// 처음 발견된 것을 쓴다. 모두 없으면 `None`(무음 no-op).
fn detect_player() -> Option<Player> {
    const CANDIDATES: &[Player] = &[
        Player {
            program: "paplay",
            args: &[],
        },
        Player {
            program: "pw-cat",
            args: &["--playback", "-"],
        },
        Player {
            program: "aplay",
            args: &["-q", "-"],
        },
    ];
    CANDIDATES.iter().find(|c| program_in_path(c.program)).copied()
}

/// PATH 상에 실행 가능한 `program` 이 있는지 확인한다.
fn program_in_path(program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| {
        let full = dir.join(program);
        full.metadata()
            .map(|m| m.is_file() && is_executable(&m))
            .unwrap_or(false)
    })
}

/// 실행 권한 비트 확인 (unix).
#[cfg(unix)]
fn is_executable(meta: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o111 != 0
}

/// unix 외 플랫폼에는 실행 권한 비트가 없다.
///
/// 이 모듈의 재생기 후보(`paplay`·`pw-cat`·`aplay`)는 전부 리눅스 오디오 스택
/// 전용이라 Windows 에서는 어차피 탐지에 실패해 무음 no-op 이 된다. 여기서 파일
/// 존재만으로 판정하는 것은 `unim-dbus` 를 Windows 타깃으로 컴파일 가능하게
/// 하려는 것이며(이 크레이트를 링크하는 `unim-cli` 의 Windows 포팅에 필요),
/// 실제 비프는 Windows 에서 TSF 의 `lang_bar.rs` 가 자체 구현한다.
#[cfg(not(unix))]
fn is_executable(_meta: &std::fs::Metadata) -> bool {
    true
}

/// 재생기 1회 탐지 캐시 — 매 비프마다 PATH 를 훑지 않도록 한다.
fn player() -> Option<Player> {
    static PLAYER: OnceLock<Option<Player>> = OnceLock::new();
    *PLAYER.get_or_init(detect_player)
}

/// 합성한 WAV 를 재생기에 넘긴다. 재생기 부재/스폰 실패 시 조용히 반환한다.
///
/// stdin 쓰기는 파이프 버퍼를 넘으면 블록될 수 있으므로 **별도 스레드**에서 쓰고,
/// 같은 스레드에서 `wait()` 로 좀비를 회수한다 — 호출한 엔진 워커 스레드는 완전
/// 논블로킹이다.
fn play(wav: Vec<u8>) {
    let Some(p) = player() else {
        return;
    };
    let mut child = match Command::new(p.program)
        .args(p.args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    if let Some(mut stdin) = child.stdin.take() {
        std::thread::spawn(move || {
            // 재생기가 조기 종료(broken pipe)해도 무해하게 무시.
            let _ = stdin.write_all(&wav);
            drop(stdin); // EOF 통지 → 재생기 종료 유도
            let _ = child.wait(); // 좀비 회수
        });
    } else {
        // stdin 확보 실패 — 좀비 회수만 별도 스레드에서.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
}

/// `ms` 밀리초에 해당하는 샘플 수.
fn ms_to_samples(ms: u32) -> u32 {
    (SAMPLE_RATE as u64 * ms as u64 / 1000) as u32
}

/// 16-bit mono 44.1kHz PCM WAV 를 메모리 합성한다.
///
/// `freq_hz` 사인파 `tone_ms` 짜리 음을 `count` 번 울리고, 음 사이에 `gap_ms` 무음을
/// 둔다. 각 음은 앞뒤 약 5ms 선형 페이드로 시작·끝 클릭음을 없앤다. 반환값은 완전한
/// RIFF/WAVE 스트림(44바이트 헤더 + PCM 데이터)이다.
fn synth_wav(freq_hz: f32, tone_ms: u32, count: u32, gap_ms: u32) -> Vec<u8> {
    let tone_samples = ms_to_samples(tone_ms);
    let gap_samples = ms_to_samples(gap_ms);
    let num_samples = tone_samples * count + gap_samples * count.saturating_sub(1);
    let fade_samples = ms_to_samples(5); // ~5ms
    let amplitude = 0.35_f32; // 클리핑 여유

    let data_bytes = num_samples * 2; // 16-bit mono
    let mut buf = Vec::with_capacity(44 + data_bytes as usize);

    // ---- RIFF 헤더 ----
    buf.extend_from_slice(b"RIFF");
    buf.extend_from_slice(&(36 + data_bytes).to_le_bytes()); // ChunkSize
    buf.extend_from_slice(b"WAVE");
    // fmt 서브청크
    buf.extend_from_slice(b"fmt ");
    buf.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (PCM)
    buf.extend_from_slice(&1u16.to_le_bytes()); // AudioFormat = PCM
    buf.extend_from_slice(&1u16.to_le_bytes()); // NumChannels = mono
    buf.extend_from_slice(&SAMPLE_RATE.to_le_bytes()); // SampleRate
    let byte_rate = SAMPLE_RATE * 2; // SampleRate * Channels(1) * BytesPerSample(2)
    buf.extend_from_slice(&byte_rate.to_le_bytes());
    buf.extend_from_slice(&2u16.to_le_bytes()); // BlockAlign = Channels*BytesPerSample
    buf.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample
    // data 서브청크
    buf.extend_from_slice(b"data");
    buf.extend_from_slice(&data_bytes.to_le_bytes());

    // ---- 샘플 ----
    let omega = std::f32::consts::TAU * freq_hz;
    for i in 0..count {
        if i > 0 {
            // 음 사이 무음
            buf.resize(buf.len() + gap_samples as usize * 2, 0);
        }
        for n in 0..tone_samples {
            let t = n as f32 / SAMPLE_RATE as f32;
            let mut s = (omega * t).sin() * amplitude;
            // 앞뒤 선형 페이드
            if fade_samples > 0 {
                if n < fade_samples {
                    s *= n as f32 / fade_samples as f32;
                }
                let tail = tone_samples - n - 1;
                if tail < fade_samples {
                    s *= tail as f32 / fade_samples as f32;
                }
            }
            let v = (s * i16::MAX as f32) as i16;
            buf.extend_from_slice(&v.to_le_bytes());
        }
    }
    buf
}

/// 한/영 모드 통지 비프 1음 길이 (ms).
const MODE_TONE_MS: u32 = 90;
/// 한/영 모드 통지 비프 음 사이 무음 (ms).
const MODE_GAP_MS: u32 = 70;
/// AutoTypeFix 토글 통지 비프 1음 길이 (ms).
const ATF_TONE_MS: u32 = 70;
/// AutoTypeFix 토글 통지 비프 음 사이 무음 (ms).
const ATF_GAP_MS: u32 = 60;

/// 한/영 모드 통지 비프. 한글=880Hz(높은 음) 2번, 영문=440Hz(낮은 음) 1번.
///
/// 높낮이를 잘 구별하지 못해도 횟수로 알 수 있다. Windows TSF
/// (`unim-tsf/src/lang_bar.rs`)의 한/영 통지 의미론과 동일하다.
pub fn announce_mode(is_korean: bool) {
    let (freq, count) = if is_korean { (880.0, 2) } else { (440.0, 1) };
    play(synth_wav(freq, MODE_TONE_MS, count, MODE_GAP_MS));
}

/// AutoTypeFix 토글 통지 비프. 켜짐=1318Hz(높은 음) 2번, 꺼짐=659Hz(낮은 음) 1번.
///
/// 한/영 비프(880/440Hz)와 음역·길이를 달리해 ATF 토글임을 구별한다.
pub fn announce_atf_toggle(on: bool) {
    let (freq, count) = if on { (1318.0, 2) } else { (659.0, 1) };
    play(synth_wav(freq, ATF_TONE_MS, count, ATF_GAP_MS));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16_le(b: &[u8], off: usize) -> u16 {
        u16::from_le_bytes([b[off], b[off + 1]])
    }
    fn u32_le(b: &[u8], off: usize) -> u32 {
        u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    }

    /// RIFF/WAVE/fmt/data 매직·PCM 포맷 파라미터가 올바른지 (오디오 장치 불필요).
    #[test]
    fn wav_header_is_valid_riff() {
        let wav = synth_wav(440.0, 90, 1, 0);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u16_le(&wav, 20), 1, "AudioFormat=PCM");
        assert_eq!(u16_le(&wav, 22), 1, "NumChannels=mono");
        assert_eq!(u32_le(&wav, 24), SAMPLE_RATE, "SampleRate=44100");
        assert_eq!(u16_le(&wav, 32), 2, "BlockAlign=2");
        assert_eq!(u16_le(&wav, 34), 16, "BitsPerSample=16");
    }

    /// 샘플 길이가 duration_ms 에 비례하고 헤더 크기 필드와 일치하는지.
    #[test]
    fn wav_sample_length_matches_duration() {
        let ms = 90;
        let wav = synth_wav(880.0, ms, 1, 0);
        let expected_bytes = (SAMPLE_RATE * ms / 1000) * 2;
        assert_eq!(wav.len() as u32, 44 + expected_bytes, "총 길이 = 헤더 44 + data");
        assert_eq!(u32_le(&wav, 40), expected_bytes, "data 청크 크기 필드");
        assert_eq!(u32_le(&wav, 4), 36 + expected_bytes, "RIFF ChunkSize");
    }

    /// 모드 비프(90ms)와 ATF 비프(70ms)는 길이가 다르다.
    #[test]
    fn mode_and_atf_wav_lengths_differ() {
        assert!(
            synth_wav(880.0, MODE_TONE_MS, 1, MODE_GAP_MS).len()
                > synth_wav(1318.0, ATF_TONE_MS, 1, ATF_GAP_MS).len()
        );
    }

    /// 여러 번 울리면 음 사이에만 무음이 들어간다: 2음 = 음 2개 + 무음 1개.
    #[test]
    fn multi_beep_length_is_tones_plus_gaps() {
        let one = ms_to_samples(MODE_TONE_MS) * 2;
        let gap = ms_to_samples(MODE_GAP_MS) * 2;
        let wav = synth_wav(880.0, MODE_TONE_MS, 2, MODE_GAP_MS);
        assert_eq!(wav.len() as u32, 44 + one * 2 + gap);
        assert_eq!(u32_le(&wav, 40), one * 2 + gap, "data 청크 크기 필드");
    }

    /// 음 사이 구간은 완전한 무음(0)이다.
    #[test]
    fn multi_beep_gap_is_silent() {
        let wav = synth_wav(880.0, MODE_TONE_MS, 2, MODE_GAP_MS);
        let start = 44 + ms_to_samples(MODE_TONE_MS) as usize * 2;
        let end = start + ms_to_samples(MODE_GAP_MS) as usize * 2;
        assert!(wav[start..end].iter().all(|&b| b == 0));
    }
}

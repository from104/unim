//! `unim-cli dict hanja` — 한자 사용자 사전(`hanja-userdict.yaml`) 관리.
//!
//! 규약: `docs/dev/specs/HANJA_USERDICT_SPEC.md` §5(CLI)·§8(오류 처리·데이터 보호).
//!
//! - 파일 직접 R/W(데몬 경유 아님). 쓰기마다 **디스크에서 다시 읽고 → 고치고 → 원자적 저장**.
//! - 읽을 수 없는 파일(파싱 실패·더 새 버전)은 **절대 덮어쓰지 않는다**. `clear` 만 파싱 실패
//!   파일을 `.broken-…` 로 보관(이름 변경)한 뒤 빈 사전으로 진행한다.
//! - 항목 내용은 로그에 남기지 않는다(이 모듈은 `unim_log!` 를 쓰지 않는다). 표준 출력에
//!   찍는 것은 사용자 본인 터미널이므로 허용.
//! - 각 연산은 `path` 를 받고 `Result<출력 문장, 오류 문장>` 을 돌려준다 — 시험이 임시 파일로
//!   실행할 수 있도록 입출력(`println!`/종료 코드)은 `run_*` 래퍼가 맡는다.

use dialoguer::{theme::ColorfulTheme, Confirm};
use rust_i18n::t;
use std::path::{Path, PathBuf};
use unim::hanja::user_dict::{EntryError, HanjaUserDict, LoadError};
use unim::input_engine::HANJA_MAX_KEY_CHARS;

use crate::{confirm_multi_removal, MultiRemoveGate};

/// 연산 결과: `Ok(표준 출력 문장)` / `Err(표준 오류 문장)`. 빈 `Err("")` 는 이미 안내를
/// 출력한 중단(사용자 취소·확인 불가)이라 문장을 더 찍지 않고 종료 코드만 1 로 한다.
type Outcome = Result<String, String>;

// ───────────────────────── 서식 ─────────────────────────

/// 터미널 제어 문자를 공백으로 바꾼다(손으로 고친 파일에 든 이스케이프 시퀀스 방어).
fn safe(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect()
}

fn pair(hangul: &str, hanja: &str) -> String {
    format!("{} → {}", safe(hangul), safe(hanja))
}

/// `EntryError` 를 사용자 문장 한 줄로 바꾼다. 글자 위치는 **1부터** 센다(엔진 값은 0부터).
/// `hangul`/`hanja` 는 오류를 낸 입력(또는 저장된 항목) 원문.
pub(crate) fn entry_error_msg(err: &EntryError, hangul: &str, hanja: &str) -> String {
    match err {
        EntryError::HangulEmpty => t!("hanja_dict_err_hangul_empty").to_string(),
        EntryError::HangulNotSyllable { ch, pos } => t!(
            "hanja_dict_err_hangul_not_syllable",
            text = safe(hangul),
            ch = safe(&ch.to_string()),
            pos = (pos + 1).to_string(),
            max = HANJA_MAX_KEY_CHARS.to_string()
        )
        .to_string(),
        EntryError::HangulTooLong { len } => t!(
            "hanja_dict_err_hangul_too_long",
            text = safe(hangul),
            len = len.to_string(),
            max = HANJA_MAX_KEY_CHARS.to_string()
        )
        .to_string(),
        EntryError::HanjaLenMismatch {
            hangul: hangul_len,
            hanja: hanja_len,
        } => t!(
            "hanja_dict_err_hanja_len",
            text = safe(hanja),
            hanja_len = hanja_len.to_string(),
            hangul_len = hangul_len.to_string()
        )
        .to_string(),
        EntryError::HanjaNotHan { ch, pos } => t!(
            "hanja_dict_err_hanja_not_han",
            text = safe(hanja),
            ch = safe(&ch.to_string()),
            code = format!("{:04X}", *ch as u32),
            pos = (pos + 1).to_string()
        )
        .to_string(),
        EntryError::MeaningTooLong { len } => t!(
            "hanja_dict_err_meaning_too_long",
            len = len.to_string(),
            max = unim::hanja::user_dict::MEANING_MAX_CHARS.to_string()
        )
        .to_string(),
        EntryError::MeaningControl => t!("hanja_dict_err_meaning_control").to_string(),
        EntryError::Duplicate => t!("hanja_dict_duplicate", pair = pair(hangul, hanja)).to_string(),
        EntryError::NotFound => t!("hanja_dict_not_found", item = pair(hangul, hanja)).to_string(),
    }
}

/// 읽기 실패 문장(SPEC §5 "파일 깨짐"·"새 버전"). 쓰기 거부 안내를 겸한다.
fn load_error_msg(err: &LoadError, path: &Path) -> String {
    match err {
        LoadError::NewerVersion(v) => {
            t!("hanja_dict_newer_version", version = v.to_string()).to_string()
        }
        LoadError::Parse { line, column, .. } => {
            let place = match (line, column) {
                (Some(l), Some(c)) => {
                    t!("hanja_dict_loc", line = l.to_string(), col = c.to_string()).to_string()
                }
                _ => t!("hanja_dict_loc_unknown").to_string(),
            };
            t!(
                "hanja_dict_corrupt",
                place = place,
                path = path.display().to_string()
            )
            .to_string()
        }
        LoadError::Io(e) => t!(
            "hanja_dict_corrupt",
            place = e.to_string(),
            path = path.display().to_string()
        )
        .to_string(),
    }
}

/// 쓰기용 읽기. 읽을 수 없으면 거부 문장(파일은 건드리지 않는다).
fn load_for_write(path: &Path) -> Result<HanjaUserDict, String> {
    HanjaUserDict::load(path).map_err(|e| load_error_msg(&e, path))
}

fn save(d: &HanjaUserDict, path: &Path) -> Result<(), String> {
    d.save(path)
        .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())
}

// ───────────────────────── 연산 ─────────────────────────

pub(crate) fn list(path: &Path, hangul: Option<&str>) -> Outcome {
    let d = HanjaUserDict::load(path).map_err(|e| load_error_msg(&e, path))?;
    let want = hangul.map(str::trim);
    let rows: Vec<usize> = (0..d.entries.len())
        .filter(|&i| want.is_none_or(|w| d.entries[i].hangul == w))
        .collect();
    if rows.is_empty() {
        return Ok(match want {
            Some(w) => t!("hanja_dict_not_found", item = safe(w)).to_string(),
            None => format!(
                "{}\n({}: {})",
                t!("hanja_dict_empty"),
                t!("user_dict_path_label"),
                path.display()
            ),
        });
    }
    let mut out = t!("hanja_dict_list_title", count = rows.len().to_string()).to_string();
    for i in rows {
        let e = &d.entries[i];
        out.push_str(&format!(
            "\n  {}. {}  {}",
            i + 1,
            safe(&e.hangul),
            safe(&e.hanja)
        ));
        if let Some(m) = e.meaning.as_deref().filter(|m| !m.is_empty()) {
            out.push_str(&format!("  — {}", safe(m)));
        }
        if let Some(err) = d.entry_error(i) {
            out.push_str(&format!(
                "\n     {}",
                t!(
                    "hanja_dict_invalid_mark",
                    reason = entry_error_msg(&err, &e.hangul, &e.hanja)
                )
            ));
        }
    }
    Ok(out)
}

pub(crate) fn add(path: &Path, hangul: &str, hanja: &str, meaning: Option<String>) -> Outcome {
    let mut d = load_for_write(path)?;
    d.add(hangul, hanja, meaning)
        .map_err(|e| entry_error_msg(&e, hangul.trim(), hanja.trim()))?;
    save(&d, path)?;
    Ok(t!(
        "hanja_dict_added",
        pair = pair(hangul.trim(), hanja.trim())
    )
    .to_string())
}

pub(crate) fn edit(
    path: &Path,
    hangul: &str,
    hanja: &str,
    new_hangul: Option<String>,
    new_hanja: Option<String>,
    meaning: Option<String>,
) -> Outcome {
    if new_hangul.is_none() && new_hanja.is_none() && meaning.is_none() {
        return Err(t!("hanja_dict_edit_nothing").to_string());
    }
    let mut d = load_for_write(path)?;
    let Some(idx) = d.find(hangul, hanja) else {
        return Err(t!(
            "hanja_dict_not_found",
            item = pair(hangul.trim(), hanja.trim())
        )
        .to_string());
    };
    let cur = d.entries[idx].clone();
    let h = new_hangul.unwrap_or(cur.hangul);
    let j = new_hanja.unwrap_or(cur.hanja);
    // `--meaning ""` 은 뜻 지움(검증이 빈 문자열을 None 으로 만든다). 옵션이 없으면 기존 뜻 유지.
    let m = match meaning {
        Some(m) => Some(m),
        None => cur.meaning,
    };
    d.update_at(idx, &h, &j, m)
        .map_err(|e| entry_error_msg(&e, h.trim(), j.trim()))?;
    save(&d, path)?;
    Ok(t!("hanja_dict_edited", pair = pair(h.trim(), j.trim())).to_string())
}

pub(crate) fn remove(path: &Path, hangul: &str, hanja: Option<&str>, yes: bool) -> Outcome {
    let mut d = load_for_write(path)?;
    let (h, j) = (hangul.trim(), hanja.map(str::trim));
    let targets: Vec<String> = d
        .entries
        .iter()
        .filter(|e| e.hangul == h && j.is_none_or(|j| e.hanja == j))
        .map(|e| pair(&e.hangul, &e.hanja))
        .collect();
    if targets.is_empty() {
        let item = match j {
            Some(j) => pair(h, j),
            None => safe(h),
        };
        return Err(t!("hanja_dict_not_found", item = item).to_string());
    }
    match confirm_multi_removal(&targets, yes) {
        MultiRemoveGate::Proceed => {}
        MultiRemoveGate::Cancelled => return Err(String::new()),
        MultiRemoveGate::NeedsYes => return Err(t!("dict_multi_remove_needs_yes").to_string()),
    }
    let n = d.remove(h, j);
    save(&d, path)?;
    Ok(match j {
        Some(j) if n == 1 => t!("hanja_dict_removed", pair = pair(h, j)).to_string(),
        _ => t!(
            "hanja_dict_removed_many",
            hangul = safe(h),
            count = n.to_string()
        )
        .to_string(),
    })
}

fn confirm(prompt: String, yes: bool) -> bool {
    yes || Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt)
        .default(false)
        .interact()
        .unwrap_or(false)
}

pub(crate) fn clear(path: &Path, yes: bool) -> Outcome {
    match HanjaUserDict::load(path) {
        Ok(mut d) => {
            if d.entries.is_empty() {
                return Ok(t!("hanja_dict_empty").to_string());
            }
            let count = d.entries.len();
            if !confirm(
                t!("hanja_dict_clear_confirm", count = count.to_string()).to_string(),
                yes,
            ) {
                println!("{}", t!("exit_canceled"));
                return Err(String::new());
            }
            d.entries.clear();
            save(&d, path)?;
            Ok(t!("hanja_dict_cleared", count = count.to_string()).to_string())
        }
        // 파싱 실패: 지우지 않고 보관(이름 변경)한 뒤 빈 사전으로 시작한다(SPEC §8).
        Err(LoadError::Parse { .. }) => {
            if !confirm(t!("hanja_dict_clear_broken_confirm").to_string(), yes) {
                println!("{}", t!("exit_canceled"));
                return Err(String::new());
            }
            let kept: PathBuf = HanjaUserDict::quarantine(path)
                .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
            Ok(t!("hanja_dict_quarantined", path = kept.display().to_string()).to_string())
        }
        // 더 새 버전·읽기 I/O 오류: 읽기 전용 — 건드리지 않는다.
        Err(e) => Err(load_error_msg(&e, path)),
    }
}

// ───────────────────────── 진입점 (입출력·종료 코드) ─────────────────────────

fn default_path_or_exit() -> PathBuf {
    match HanjaUserDict::default_path() {
        Some(p) => p,
        None => {
            eprintln!("{}", t!("error_path_not_found"));
            std::process::exit(1);
        }
    }
}

fn finish(res: Outcome) {
    match res {
        Ok(s) => println!("{s}"),
        Err(s) => {
            if !s.is_empty() {
                eprintln!("{}: {}", t!("error_label"), s);
            }
            std::process::exit(1);
        }
    }
}

pub(crate) fn run_list(hangul: Option<String>) {
    finish(list(&default_path_or_exit(), hangul.as_deref()));
}

pub(crate) fn run_add(hangul: String, hanja: String, meaning: Option<String>) {
    finish(add(&default_path_or_exit(), &hangul, &hanja, meaning));
}

pub(crate) fn run_edit(
    hangul: String,
    hanja: String,
    new_hangul: Option<String>,
    new_hanja: Option<String>,
    meaning: Option<String>,
) {
    finish(edit(
        &default_path_or_exit(),
        &hangul,
        &hanja,
        new_hangul,
        new_hanja,
        meaning,
    ));
}

pub(crate) fn run_remove(hangul: String, hanja: Option<String>, yes: bool) {
    finish(remove(
        &default_path_or_exit(),
        &hangul,
        hanja.as_deref(),
        yes,
    ));
}

pub(crate) fn run_clear(yes: bool) {
    finish(clear(&default_path_or_exit(), yes));
}

pub(crate) fn run_path() {
    println!("{}", default_path_or_exit().display());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "unim-cli-hanja-test-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir.join("hanja-userdict.yaml")
    }

    fn cleanup(p: &Path) {
        let _ = fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn add_list_edit_remove_roundtrip() {
        let p = tmp("roundtrip");
        add(&p, "서기현", "徐基鉉", Some("내 이름".into())).unwrap();
        // 중복
        let e = add(&p, "서기현", "徐基鉉", None).unwrap_err();
        assert!(e.contains("서기현 → 徐基鉉"), "{e}");
        let out = list(&p, None).unwrap();
        assert!(out.contains("1. 서기현  徐基鉉  — 내 이름"), "{out}");
        // 필터
        assert!(list(&p, Some("없음")).unwrap().contains("없음"));
        // 뜻만 지움
        edit(&p, "서기현", "徐基鉉", None, None, Some(String::new())).unwrap();
        let d = HanjaUserDict::load(&p).unwrap();
        assert_eq!(d.entries[0].meaning, None);
        // 한자 바꿈 — 위치 유지
        edit(&p, "서기현", "徐基鉉", None, Some("徐基炫".into()), None).unwrap();
        assert_eq!(HanjaUserDict::load(&p).unwrap().entries[0].hanja, "徐基炫");
        // 옵션 없음
        assert!(edit(&p, "서기현", "徐基炫", None, None, None).is_err());
        // 없는 쌍
        assert!(edit(&p, "서기현", "徐基鉉", None, None, Some("x".into())).is_err());
        remove(&p, "서기현", Some("徐基炫"), false).unwrap();
        assert!(HanjaUserDict::load(&p).unwrap().entries.is_empty());
        assert!(remove(&p, "서기현", None, true).is_err());
        cleanup(&p);
    }

    #[test]
    fn remove_all_of_hangul_needs_yes_when_multiple() {
        let p = tmp("remove-multi");
        add(&p, "가", "家", None).unwrap();
        add(&p, "가", "加", None).unwrap();
        add(&p, "나", "那", None).unwrap();
        // 비대화형(시험 stdin)에서 -y 없이 2개 이상은 거부, 파일 그대로
        let e = remove(&p, "가", None, false).unwrap_err();
        assert!(!e.is_empty());
        assert_eq!(HanjaUserDict::load(&p).unwrap().entries.len(), 3);
        remove(&p, "가", None, true).unwrap();
        let d = HanjaUserDict::load(&p).unwrap();
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.entries[0].hangul, "나");
        cleanup(&p);
    }

    #[test]
    fn corrupt_file_is_never_overwritten() {
        let p = tmp("corrupt");
        fs::write(&p, "entries: [\n").unwrap();
        let before = fs::read(&p).unwrap();
        let e = add(&p, "가", "家", None).unwrap_err();
        assert!(e.contains("clear"), "{e}");
        assert!(edit(&p, "가", "家", None, None, Some("x".into())).is_err());
        assert!(remove(&p, "가", None, true).is_err());
        assert!(list(&p, None).is_err());
        assert_eq!(fs::read(&p).unwrap(), before);
        // clear 만 보관 후 비운다(원본은 .broken-* 로 이름만 바뀜)
        let msg = clear(&p, true).unwrap();
        assert!(msg.contains(".broken-"), "{msg}");
        assert!(!p.exists());
        let kept: Vec<_> = fs::read_dir(p.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        assert_eq!(kept.len(), 1);
        assert_eq!(fs::read(kept[0].path()).unwrap(), before);
        // 이후 정상 추가
        add(&p, "가", "家", None).unwrap();
        cleanup(&p);
    }

    #[test]
    fn newer_version_is_read_only() {
        let p = tmp("newer");
        fs::write(&p, "version: 9\nentries: []\n").unwrap();
        let before = fs::read(&p).unwrap();
        assert!(add(&p, "가", "家", None).is_err());
        assert!(clear(&p, true).is_err());
        assert_eq!(fs::read(&p).unwrap(), before);
        cleanup(&p);
    }

    #[test]
    fn clear_removes_all_after_yes() {
        let p = tmp("clear");
        add(&p, "가", "家", None).unwrap();
        add(&p, "나", "那", None).unwrap();
        let msg = clear(&p, true).unwrap();
        assert!(msg.contains('2'), "{msg}");
        assert!(HanjaUserDict::load(&p).unwrap().entries.is_empty());
        cleanup(&p);
    }

    #[test]
    fn list_marks_invalid_entries_and_keeps_them() {
        let p = tmp("invalid");
        fs::write(
            &p,
            "version: 1\nentries:\n- hangul: 가나다\n  hanja: 家那\n- hangul: 라\n  hanja: 羅\n",
        )
        .unwrap();
        let out = list(&p, None).unwrap();
        assert!(out.contains("⚠"), "{out}");
        // 무효 항목도 편집·삭제 가능
        remove(&p, "가나다", Some("家那"), false).unwrap();
        assert_eq!(HanjaUserDict::load(&p).unwrap().entries.len(), 1);
        cleanup(&p);
    }

    #[test]
    fn validation_messages_use_one_based_positions() {
        let p = tmp("msgs");
        // 3번째 글자가 한자 아님
        let e = add(&p, "서기현", "徐基A", None).unwrap_err();
        assert!(e.contains('3') && e.contains("U+0041"), "{e}");
        // 글자 수
        let e = add(&p, "서기현", "徐基", None).unwrap_err();
        assert!(e.contains('2') && e.contains('3'), "{e}");
        // 한글 2번째 글자가 자모
        let e = add(&p, "가ㄱ", "家家", None).unwrap_err();
        assert!(e.contains('2') && e.contains('ㄱ'), "{e}");
        // 뜻
        let long = "뜻".repeat(61);
        assert!(add(&p, "가", "家", Some(long)).is_err());
        assert!(add(&p, "가", "家", Some("a\nb".into())).is_err());
        // 실패는 파일을 만들지 않는다
        assert!(!p.exists());
        cleanup(&p);
    }

    #[test]
    fn safe_strips_control_chars() {
        assert_eq!(safe("a\x1b[31mb\n"), "a [31mb ");
    }
}

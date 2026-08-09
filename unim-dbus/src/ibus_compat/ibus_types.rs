//! IBus GVariant 직렬화 타입
//!
//! IBus 프로토콜의 CommitText, UpdatePreeditText 시그널은
//! IBusSerializable 기반의 커스텀 GVariant 형식을 사용한다.
//!
//! ## IBusSerializable 직렬화 형식
//!
//! 모든 IBus 객체는 `(sa{sv}...)` 구조:
//! - `s`: 타입 이름 (예: "IBusText")
//! - `a{sv}`: 첨부 데이터 (보통 빈 dict)
//! - `...`: 타입별 추가 필드
//!
//! ## 참고
//!
//! IBus 소스: <https://github.com/ibus/ibus/blob/main/src/ibustext.c>

use zbus::zvariant::{self, Value};

/// IBusAttribute 타입 상수
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum IBusAttrType {
    /// 밑줄 스타일
    Underline = 1,
    /// 전경색
    Foreground = 2,
    /// 배경색
    Background = 3,
}

/// IBusAttribute 밑줄 스타일 상수
#[repr(u32)]
#[derive(Debug, Clone, Copy)]
pub enum IBusAttrUnderline {
    /// 밑줄 없음
    None = 0,
    /// 단일 밑줄
    Single = 1,
    /// 이중 밑줄
    Double = 2,
    /// 아래쪽 밑줄 (낮은 위치)
    Low = 3,
    /// 에러 밑줄 (물결)
    Error = 4,
}

/// IBus Capabilities 비트마스크
#[allow(dead_code)]
pub mod caps {
    pub const PREEDIT_TEXT: u32 = 1 << 0;
    pub const AUX_STRING: u32 = 1 << 1;
    pub const LOOKUP_TABLE: u32 = 1 << 2;
    pub const FOCUS: u32 = 1 << 3;
    pub const PROPERTY: u32 = 1 << 4;
    pub const SURROUNDING_TEXT: u32 = 1 << 5;
}

/// 빈 attachments dict `a{sv}` 생성
///
/// ⚠️ `Value::new(..)` 로 감싸지 **않는다**. 구조체 필드를 `Value` 로 넣으면
/// zvariant 가 그 자리를 `v`(variant) 로 만들어 버려 시그니처가 IBus 계약과
/// 어긋난다. IBus 클라이언트는 `a{sv}` 를 기대하므로 맵을 그대로 넣어야 한다.
fn empty_attachments() -> std::collections::HashMap<String, Value<'static>> {
    std::collections::HashMap::new()
}

/// IBusAttribute를 GVariant `v`로 직렬화
///
/// 형식: `(sa{sv}uuuu)`
/// - "IBusAttribute", {}, type, value, start_index, end_index
pub fn serialize_ibus_attribute(
    attr_type: IBusAttrType,
    value: u32,
    start: u32,
    end: u32,
) -> Value<'static> {
    Value::new(zvariant::Structure::from((
        "IBusAttribute".to_string(),
        empty_attachments(),
        attr_type as u32,
        value,
        start,
        end,
    )))
}

/// IBusAttrList를 GVariant `v`로 직렬화
///
/// 형식: `(sa{sv}av)`
/// - "IBusAttrList", {}, [IBusAttribute, ...]
pub fn serialize_ibus_attr_list(attrs: Vec<Value<'static>>) -> Value<'static> {
    Value::new(zvariant::Structure::from((
        "IBusAttrList".to_string(),
        empty_attachments(),
        attrs,
    )))
}

/// 빈 IBusAttrList 직렬화
pub fn empty_attr_list() -> Value<'static> {
    serialize_ibus_attr_list(Vec::new())
}

/// IBusText를 GVariant `v`로 직렬화
///
/// 형식: `(sa{sv}sv)`
/// - "IBusText", {}, text_string, IBusAttrList
///
/// CommitText, UpdatePreeditText 시그널의 파라미터로 사용된다.
pub fn serialize_ibus_text(text: &str) -> Value<'static> {
    serialize_ibus_text_with_attrs(text, empty_attr_list())
}

/// 속성 포함 IBusText 직렬화
pub fn serialize_ibus_text_with_attrs(text: &str, attr_list: Value<'static>) -> Value<'static> {
    Value::new(zvariant::Structure::from((
        "IBusText".to_string(),
        empty_attachments(),
        text.to_string(),
        attr_list, // 여기는 IBus 계약도 `v` 다 — Value 를 유지한다
    )))
}

/// 조합 중인 preedit 텍스트 직렬화 (밑줄 속성 포함)
///
/// IBus 관례: preedit 전체에 UNDERLINE_SINGLE 적용
pub fn serialize_preedit_text(text: &str) -> Value<'static> {
    if text.is_empty() {
        return serialize_ibus_text("");
    }

    // IBus attribute 의 start/end 는 **문자 인덱스**다(바이트 아님).
    // 바이트 길이를 넣으면 한글에서 범위를 벗어나 클라이언트가 텍스트를 버린다
    // — 2026-08-09 실측: preedit "한" 이 화면에 빈 문자열로 도착했다.
    let char_len = text.chars().count() as u32;
    let underline_attr = serialize_ibus_attribute(
        IBusAttrType::Underline,
        IBusAttrUnderline::Single as u32,
        0,
        char_len,
    );
    let attr_list = serialize_ibus_attr_list(vec![underline_attr]);
    serialize_ibus_text_with_attrs(text, attr_list)
}

/// IBusEngineDesc를 GVariant `v`로 직렬화 (ListEngines, GetGlobalEngine용)
///
/// 형식: `(sa{sv}ssssssssssssss)`
pub fn serialize_engine_desc() -> Value<'static> {
    Value::new(zvariant::Structure::from((
        "IBusEngineDesc".to_string(),
        empty_attachments(),
        "unim".to_string(),                // name
        "UNIM".to_string(),                // longname
        "Korean Input Method".to_string(), // description
        "ko".to_string(),                  // language
        "MIT".to_string(),                 // license
        "from104".to_string(),             // author
        "".to_string(),                    // icon
        "default".to_string(),             // layout
        "".to_string(),                    // layout_variant
        "".to_string(),                    // layout_option
        "0".to_string(),                   // rank
        "".to_string(),                    // hotkeys
        "unim".to_string(),                // symbol
        "".to_string(),                    // setup
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Value에서 문자열 추출 (중첩 Value 처리)
    fn extract_str<'a>(v: &'a Value<'a>) -> &'a str {
        match v {
            Value::Str(s) => s.as_str(),
            Value::Value(inner) => extract_str(inner),
            _ => panic!("Expected Str, got {:?}", v),
        }
    }

    /// Structure의 필드에서 문자열 추출
    fn field_str<'a>(fields: &'a [Value<'a>], idx: usize) -> &'a str {
        extract_str(&fields[idx])
    }

    /// Value에서 Structure 필드 추출
    fn as_struct_fields<'a>(v: &'a Value<'a>) -> &'a [Value<'a>] {
        match v {
            Value::Structure(s) => s.fields(),
            Value::Value(inner) => as_struct_fields(inner),
            _ => panic!("Expected Structure, got {:?}", v),
        }
    }

    /// wire 시그니처 검증 — **이것이 진짜 판정 기준이다.**
    ///
    /// 값만 보는 단언(`extract_str`)은 `Value::Value(inner)` 를 벗겨 내므로
    /// 필드가 variant 로 잘못 감싸져 있어도 통과한다. 실제로 2026-08-09 까지
    /// 모든 필드가 `v` 로 나가 IBus 클라이언트가
    /// `format string '&s' has a type of 's' but the given value has a type of 'v'`
    /// 로 거부했는데도 이 파일의 테스트는 전부 초록이었다.
    fn sig_of(v: &Value<'_>) -> String {
        match v {
            Value::Value(inner) => inner.value_signature().to_string(),
            other => other.value_signature().to_string(),
        }
    }

    #[test]
    fn test_ibus_text_wire_signature() {
        // IBusText = (sa{sv}sv) — IBus 의 ibus_text_serialize 계약
        assert_eq!(sig_of(&serialize_ibus_text("한글")), "(sa{sv}sv)");
        assert_eq!(sig_of(&serialize_ibus_text("")), "(sa{sv}sv)");
        assert_eq!(sig_of(&serialize_preedit_text("한")), "(sa{sv}sv)");
    }

    #[test]
    fn test_ibus_attr_list_wire_signature() {
        // IBusAttrList = (sa{sv}av)
        assert_eq!(sig_of(&empty_attr_list()), "(sa{sv}av)");
    }

    #[test]
    fn test_ibus_attribute_wire_signature() {
        // IBusAttribute = (sa{sv}uuuu)
        let attr = serialize_ibus_attribute(IBusAttrType::Underline, 1, 0, 3);
        assert_eq!(sig_of(&attr), "(sa{sv}uuuu)");
    }

    #[test]
    fn test_serialize_ibus_text_simple() {
        let value = serialize_ibus_text("한글");
        let fields = as_struct_fields(&value);
        assert_eq!(field_str(fields, 0), "IBusText");
        assert_eq!(field_str(fields, 2), "한글");
    }

    #[test]
    fn test_serialize_ibus_text_empty() {
        let value = serialize_ibus_text("");
        let fields = as_struct_fields(&value);
        assert_eq!(field_str(fields, 2), "");
    }

    #[test]
    fn test_serialize_preedit_text_with_underline() {
        let value = serialize_preedit_text("한");
        let fields = as_struct_fields(&value);
        assert_eq!(field_str(fields, 0), "IBusText");
        assert_eq!(field_str(fields, 2), "한");
        // IBusAttrList
        let al_fields = as_struct_fields(&fields[3]);
        assert_eq!(field_str(al_fields, 0), "IBusAttrList");
        // attrs 배열에 1개 항목
        match &al_fields[2] {
            Value::Array(arr) => assert_eq!(arr.len(), 1),
            Value::Value(inner) => {
                if let Value::Array(arr) = inner.as_ref() {
                    assert_eq!(arr.len(), 1);
                } else {
                    panic!("Expected Array for attrs");
                }
            }
            _ => panic!("Expected Array for attrs"),
        }
    }

    #[test]
    fn test_serialize_preedit_text_empty() {
        let value = serialize_preedit_text("");
        let fields = as_struct_fields(&value);
        assert_eq!(field_str(fields, 2), "");
    }

    #[test]
    fn test_serialize_engine_desc() {
        let value = serialize_engine_desc();
        let fields = as_struct_fields(&value);
        assert_eq!(field_str(fields, 0), "IBusEngineDesc");
        assert_eq!(field_str(fields, 2), "unim");
    }

    #[test]
    fn test_empty_attr_list() {
        let value = empty_attr_list();
        let fields = as_struct_fields(&value);
        assert_eq!(field_str(fields, 0), "IBusAttrList");
        match &fields[2] {
            Value::Array(arr) => assert_eq!(arr.len(), 0),
            Value::Value(inner) => {
                if let Value::Array(arr) = inner.as_ref() {
                    assert_eq!(arr.len(), 0);
                } else {
                    panic!("Expected Array");
                }
            }
            _ => panic!("Expected Array"),
        }
    }
}

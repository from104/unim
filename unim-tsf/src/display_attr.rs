//! Display Attributes — preedit 밑줄 스타일링

use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;
use windows::core::*;

use crate::globals;

// ── Input (조합 중) 디스플레이 속성 ──

#[implement(ITfDisplayAttributeInfo)]
pub struct InputDisplayAttribute;

impl InputDisplayAttribute {
    pub fn new() -> Self { Self }
}

impl ITfDisplayAttributeInfo_Impl for InputDisplayAttribute_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(globals::UNIM_DISPLAY_ATTR_INPUT)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("UNIM Input"))
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        unsafe {
            if !pda.is_null() {
                *pda = TF_DISPLAYATTRIBUTE {
                    crText: TF_DA_COLOR {
                        r#type: TF_DA_COLORTYPE(0),
                        Anonymous: TF_DA_COLOR_0 { nIndex: 0 },
                    },
                    crBk: TF_DA_COLOR {
                        r#type: TF_DA_COLORTYPE(0),
                        Anonymous: TF_DA_COLOR_0 { nIndex: 0 },
                    },
                    crLine: TF_DA_COLOR {
                        r#type: TF_DA_COLORTYPE(2),
                        Anonymous: TF_DA_COLOR_0 { cr: COLORREF(0x00C86400) },
                    },
                    lsStyle: TF_DA_LINESTYLE(1), // TF_LS_SOLID
                    fBoldLine: FALSE,
                    bAttr: TF_DA_ATTR_INFO(0), // TF_ATTR_INPUT
                };
            }
        }
        Ok(())
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> { Ok(()) }
    fn Reset(&self) -> Result<()> { Ok(()) }
}

// ── Converted (변환 완료) 디스플레이 속성 ──

#[implement(ITfDisplayAttributeInfo)]
pub struct ConvertedDisplayAttribute;

impl ConvertedDisplayAttribute {
    pub fn new() -> Self { Self }
}

impl ITfDisplayAttributeInfo_Impl for ConvertedDisplayAttribute_Impl {
    fn GetGUID(&self) -> Result<GUID> {
        Ok(globals::UNIM_DISPLAY_ATTR_CONVERTED)
    }

    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("UNIM Converted"))
    }

    fn GetAttributeInfo(&self, pda: *mut TF_DISPLAYATTRIBUTE) -> Result<()> {
        unsafe {
            if !pda.is_null() {
                *pda = TF_DISPLAYATTRIBUTE {
                    crText: TF_DA_COLOR {
                        r#type: TF_DA_COLORTYPE(0),
                        Anonymous: TF_DA_COLOR_0 { nIndex: 0 },
                    },
                    crBk: TF_DA_COLOR {
                        r#type: TF_DA_COLORTYPE(2),
                        Anonymous: TF_DA_COLOR_0 { cr: COLORREF(0x00FFFF00) },
                    },
                    crLine: TF_DA_COLOR {
                        r#type: TF_DA_COLORTYPE(0),
                        Anonymous: TF_DA_COLOR_0 { nIndex: 0 },
                    },
                    lsStyle: TF_DA_LINESTYLE(0), // TF_LS_NONE
                    fBoldLine: FALSE,
                    bAttr: TF_DA_ATTR_INFO(2), // TF_ATTR_TARGET_CONVERTED
                };
            }
        }
        Ok(())
    }

    fn SetAttributeInfo(&self, _pda: *const TF_DISPLAYATTRIBUTE) -> Result<()> { Ok(()) }
    fn Reset(&self) -> Result<()> { Ok(()) }
}

// ── Display Attribute 열거자 ──

#[implement(IEnumTfDisplayAttributeInfo)]
pub struct DisplayAttributeEnum {
    index: std::sync::Mutex<usize>,
}

impl DisplayAttributeEnum {
    pub fn new() -> Self {
        Self { index: std::sync::Mutex::new(0) }
    }
}

impl IEnumTfDisplayAttributeInfo_Impl for DisplayAttributeEnum_Impl {
    fn Clone(&self) -> Result<IEnumTfDisplayAttributeInfo> {
        let cloned = DisplayAttributeEnum::new();
        *cloned.index.lock().unwrap() = *self.index.lock().unwrap();
        Ok(cloned.into())
    }

    fn Next(
        &self,
        ulcount: u32,
        rginfo: *mut Option<ITfDisplayAttributeInfo>,
        pcfetched: *mut u32,
    ) -> Result<()> {
        unsafe {
            let mut fetched = 0u32;
            let mut idx = self.index.lock().unwrap();

            for i in 0..ulcount {
                let info: Option<ITfDisplayAttributeInfo> = match *idx {
                    0 => Some(InputDisplayAttribute::new().into()),
                    1 => Some(ConvertedDisplayAttribute::new().into()),
                    _ => None,
                };
                if let Some(attr) = info {
                    *rginfo.add(i as usize) = Some(attr);
                    *idx += 1;
                    fetched += 1;
                } else {
                    break;
                }
            }

            if !pcfetched.is_null() {
                *pcfetched = fetched;
            }

            if fetched == ulcount { Ok(()) } else { Err(S_FALSE.into()) }
        }
    }

    fn Reset(&self) -> Result<()> {
        *self.index.lock().unwrap() = 0;
        Ok(())
    }

    fn Skip(&self, ulcount: u32) -> Result<()> {
        *self.index.lock().unwrap() += ulcount as usize;
        Ok(())
    }
}

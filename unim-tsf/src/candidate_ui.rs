//! 한자/특수문자 후보 창 — ITfCandidateListUIElement 기본 구현

use std::sync::Mutex;

use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::UI::TextServices::*;

#[implement(ITfCandidateListUIElement, ITfUIElement)]
pub struct UnimCandidateList {
    candidates: Mutex<Vec<String>>,
    selection: Mutex<u32>,
    page_start: Mutex<u32>,
    visible: Mutex<bool>,
}

impl UnimCandidateList {
    pub fn new() -> Self {
        Self {
            candidates: Mutex::new(Vec::new()),
            selection: Mutex::new(0),
            page_start: Mutex::new(0),
            visible: Mutex::new(false),
        }
    }

    pub fn set_candidates(&self, candidates: Vec<String>) {
        *self.candidates.lock().unwrap() = candidates;
        *self.selection.lock().unwrap() = 0;
        *self.page_start.lock().unwrap() = 0;
    }

    pub fn set_visible(&self, visible: bool) {
        *self.visible.lock().unwrap() = visible;
    }
}

// ── ITfUIElement ──

impl ITfUIElement_Impl for UnimCandidateList_Impl {
    fn GetDescription(&self) -> Result<BSTR> {
        Ok(BSTR::from("UNIM Candidate List"))
    }

    fn GetGUID(&self) -> Result<GUID> {
        Ok(crate::globals::UNIM_CLSID)
    }

    fn Show(&self, bshow: BOOL) -> Result<()> {
        *self.visible.lock().unwrap() = bshow.as_bool();
        Ok(())
    }

    fn IsShown(&self) -> Result<BOOL> {
        Ok(BOOL::from(*self.visible.lock().unwrap()))
    }
}

// ── ITfCandidateListUIElement ──

impl ITfCandidateListUIElement_Impl for UnimCandidateList_Impl {
    fn GetUpdatedFlags(&self) -> Result<u32> {
        Ok(0x1 | 0x4 | 0x10) // TF_CLUIE_STRING | TF_CLUIE_SELECTION | TF_CLUIE_COUNT
    }

    fn GetDocumentMgr(&self) -> Result<ITfDocumentMgr> {
        Err(E_NOTIMPL.into())
    }

    fn GetCount(&self) -> Result<u32> {
        Ok(self.candidates.lock().unwrap().len() as u32)
    }

    fn GetSelection(&self) -> Result<u32> {
        Ok(*self.selection.lock().unwrap())
    }

    fn GetString(&self, uindex: u32) -> Result<BSTR> {
        let candidates = self.candidates.lock().unwrap();
        if (uindex as usize) < candidates.len() {
            Ok(BSTR::from(&candidates[uindex as usize]))
        } else {
            Err(E_INVALIDARG.into())
        }
    }

    fn GetPageIndex(&self, pindex: *mut u32, usize_: u32, pupagecnt: *mut u32) -> Result<()> {
        unsafe {
            let count = self.candidates.lock().unwrap().len() as u32;
            let page_size = 9u32;
            let page_count = (count + page_size - 1) / page_size;

            if !pupagecnt.is_null() {
                *pupagecnt = page_count;
            }

            if !pindex.is_null() {
                for i in 0..usize_.min(page_count) {
                    *pindex.add(i as usize) = i * page_size;
                }
            }
        }
        Ok(())
    }

    fn SetPageIndex(&self, _pindex: *const u32, _upagecnt: u32) -> Result<()> {
        Ok(())
    }

    fn GetCurrentPage(&self) -> Result<u32> {
        let sel = *self.selection.lock().unwrap();
        Ok(sel / 9)
    }
}

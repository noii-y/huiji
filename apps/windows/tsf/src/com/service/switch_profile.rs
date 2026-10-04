//! 退出灰迹时切到一个非灰迹的输入法 profile。
//! 先用系统默认 profile；若默认是灰迹（安装时可能被设为默认），则按枚举顺序
//! 选第一个非灰迹的键盘 TIP（标准系统上微软拼音排在微软五笔之前）。

use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance};
use windows::Win32::UI::Input::KeyboardAndMouse::HKL;
use windows::Win32::UI::TextServices::{
    CLSID_TF_InputProcessorProfiles, GUID_TFCAT_TIP_KEYBOARD, ITfInputProcessorProfileMgr,
    ITfInputProcessorProfiles, TF_IPPMF_ENABLEPROFILE, TF_IPPMF_FORSESSION, TF_LANGUAGEPROFILE,
};
use windows::core::{GUID, Interface, Result};

use crate::com::CLSID_QINGJIAN;

/// 选中并激活一个非灰迹的输入法 profile。
pub(crate) fn switch_to_default() -> Result<()> {
    let profiles: ITfInputProcessorProfiles =
        unsafe { CoCreateInstance(&CLSID_TF_InputProcessorProfiles, None, CLSCTX_INPROC_SERVER)? };
    let langid = unsafe { profiles.GetCurrentLanguage() }?;

    // 1. 系统默认 profile 若不是灰迹，直接用它。
    let mut def_clsid = GUID::zeroed();
    let mut def_profile = GUID::zeroed();
    let default_usable = unsafe {
        profiles.GetDefaultLanguageProfile(
            langid,
            &GUID_TFCAT_TIP_KEYBOARD,
            &mut def_clsid,
            &mut def_profile,
        )
    }
    .is_ok()
        && def_clsid != CLSID_QINGJIAN;

    let mut target: Option<(GUID, GUID)> = if default_usable {
        Some((def_clsid, def_profile))
    } else {
        None
    };

    // 2. 兜底：枚举，按顺序取第一个非灰迹键盘 TIP；没有再放宽到任意非灰迹项。
    if target.is_none() {
        let enumerator = unsafe { profiles.EnumLanguageProfiles(langid) }?;
        let mut first_tip: Option<(GUID, GUID)> = None;
        let mut first_any: Option<(GUID, GUID)> = None;
        loop {
            let mut buf = [TF_LANGUAGEPROFILE::default()];
            let mut fetched = 0u32;
            if unsafe { enumerator.Next(&mut buf, &mut fetched) }.is_ok() && fetched > 0 {
                let item = buf[0];
                if item.clsid == CLSID_QINGJIAN {
                    continue;
                }
                if first_any.is_none() {
                    first_any = Some((item.clsid, item.guidProfile));
                }
                if item.catid == GUID_TFCAT_TIP_KEYBOARD && first_tip.is_none() {
                    first_tip = Some((item.clsid, item.guidProfile));
                }
            } else {
                break;
            }
        }
        target = first_tip.or(first_any);
    }

    let (clsid, guid_profile) =
        target.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))?;

    let manager: ITfInputProcessorProfileMgr = profiles.cast()?;
    // FORSESSION：对当前桌面所有线程生效；ENABLEPROFILE：目标若未在注册表启用，先启用再激活。
    unsafe {
        manager.ActivateProfile(
            1, // TF_PROFILETYPE_INPUTPROCESSOR
            langid,
            &clsid,
            &guid_profile,
            HKL::default(),
            TF_IPPMF_FORSESSION | TF_IPPMF_ENABLEPROFILE,
        )
    }
}

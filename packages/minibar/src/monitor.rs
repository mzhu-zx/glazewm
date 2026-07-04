use anyhow::Result;
use windows::{
  Win32::{
    Foundation::{LPARAM, RECT},
    Graphics::Gdi::{
      EnumDisplayMonitors, GetMonitorInfoA, HDC, HMONITOR, MONITORINFO,
      MONITORINFOEXA,
    },
    UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
  },
  core::BOOL,
};

/// DPI corresponding to a 100% scale factor.
const DEFAULT_DPI: u32 = 96;

unsafe extern "system" fn monitor_enum_proc(
  hmonitor: HMONITOR,
  _hdc: HDC,
  lprc_monitor: *mut RECT,
  dw_data: LPARAM,
) -> BOOL {
  let rects: &mut Vec<MonitorInfo> =
    unsafe { &mut *(dw_data.0 as *mut Vec<_>) };

  let info = {
    let mut out: MONITORINFOEXA = Default::default();
    out.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXA>() as _;
    unsafe {
      GetMonitorInfoA(hmonitor, &mut out as *mut _ as *mut MONITORINFO)
    }
    .unwrap();
    out
  };

  let dev_name = unsafe {
    String::from_utf8_lossy(
      (&info.szDevice as *const [i8; 32] as *const [u8; 32])
        .as_ref()
        .unwrap(),
    )
    .trim_end_matches('\0')
    .to_string()
  };

  // Effective DPI of the monitor, accounting for the user's display scale.
  // Falls back to `DEFAULT_DPI` (100%) if the query fails.
  let dpi = {
    let mut dpi_x = DEFAULT_DPI;
    let mut dpi_y = DEFAULT_DPI;
    // SAFETY: `hmonitor` is a valid handle provided by the enumeration.
    if unsafe {
      GetDpiForMonitor(hmonitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y)
    }
    .is_ok()
    {
      dpi_x
    } else {
      DEFAULT_DPI
    }
  };

  if let Some(mon) = unsafe { lprc_monitor.as_ref() } {
    rects.push(MonitorInfo {
      rect: *mon,
      dev_name,
      dpi,
    });
  }

  BOOL(1)
}

#[derive(Debug)]
pub struct MonitorInfo {
  pub rect: RECT,
  pub dev_name: String,
  /// Effective DPI of the monitor (`96` is 100% scaling).
  pub dpi: u32,
}

impl MonitorInfo {
  /// Scale factor of the monitor, where `1.0` is 100% scaling (96 DPI).
  ///
  /// Design (logical) sizes are multiplied by this to obtain physical
  /// device pixels.
  pub fn scale_factor(&self) -> f32 {
    self.dpi as f32 / DEFAULT_DPI as f32
  }
}

pub fn enum_monitor_infos() -> Result<Vec<MonitorInfo>> {
  let mut rects = vec![];

  unsafe {
    EnumDisplayMonitors(
      None,
      None,
      Some(monitor_enum_proc),
      LPARAM((&mut rects as *mut Vec<_>) as isize),
    )
    .ok()?; // convert BOOL failure into windows::core::Error
  }

  Ok(rects)
}

// impl MonitorInfo {
//   pub fn update_monitor_info(&self) -> Option<Self> {
//     let mut ret = MonitorInfo {
//       rect: Default::default(),
//       dev_name: self.dev_name.clone(),
//     };

//     let found = unsafe {
//       EnumDisplayMonitors(
//         None,
//         None,
//         Some(update_monitor_enum_proc),
//         LPARAM((&mut ret as *mut _) as isize),
//       )
//       .0 == 0
//     };
//     found.then_some(ret)
//   }
// }

// unsafe extern "system" fn update_monitor_enum_proc(
//   hmonitor: HMONITOR,
//   _hdc: HDC,
//   lprc_monitor: *mut RECT,
//   dw_data: LPARAM,
// ) -> BOOL {
//   let current_monitor_info: &mut MonitorInfo =
//     unsafe { &mut *(dw_data.0 as *mut _) };

//   let info = {
//     let mut out: MONITORINFOEXA = Default::default();
//     out.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXA>() as _;
//     unsafe {
//       GetMonitorInfoA(hmonitor, &mut out as *mut _ as *mut MONITORINFO)
//     }
//     .unwrap();
//     out
//   };

//   let dev_name = unsafe {
//     String::from_utf8_lossy(
//       (&info.szDevice as *const [i8; 32] as *const [u8; 32])
//         .as_ref()
//         .unwrap(),
//     )
//     .trim_end_matches('\0')
//     .to_string()
//   };

//   if dev_name == current_monitor_info.dev_name {
//     if let Some(mon) = unsafe { lprc_monitor.as_ref() } {
//       current_monitor_info.rect = *mon;
//     }
//     BOOL(0)
//   } else {
//     BOOL(1)
//   }
// }

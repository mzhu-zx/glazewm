use anyhow::Result;
use windows::{
  Win32::{
    Foundation::{LPARAM, RECT},
    Graphics::Gdi::{
      EnumDisplayMonitors, GetMonitorInfoA, HDC,
      HMONITOR, MONITORINFO, MONITORINFOEXA,
    },
  },
  core::BOOL,
};

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
  }
  .to_string();

  if let Some(mon) = unsafe { lprc_monitor.as_ref() } {
    rects.push(MonitorInfo {
      rect: *mon,
      dev_name,
    });
  }

  BOOL(1)
}

#[derive(Debug)]
pub struct MonitorInfo {
  pub rect: RECT,
  pub dev_name: String,
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

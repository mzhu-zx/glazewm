// #![windows_subsystem = "windows"]

use anyhow::Result;
use tracing::info;
use windows::{
  Win32::{
    Foundation::{GetLastError, LPARAM, RECT},
    Graphics::Gdi::{
      EnumDisplayMonitors, GetMonitorInfoA, GetMonitorInfoW, HDC,
      HMONITOR, MONITORINFO, MONITORINFOEXA,
    },
    System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW},
    UI::HiDpi::GetProcessDpiAwareness,
  },
  core::BOOL,
};

use crate::{
  logging::init_logger,
  minibar::{Config, Minibar},
  monitor::enum_monitor_infos,
  watcher::GlazeWmService,
};

// mod compositor;
mod color;
mod d2d;
mod logging;
mod minibar;
mod monitor;
mod watcher;
mod window;

#[tokio::main]
async fn main() {
  init_logger();

  let dpi_awareness = unsafe { GetProcessDpiAwareness(None) }.unwrap();
  info!("dpi awareness: {:?}", dpi_awareness);
  let hinst = unsafe { GetModuleHandleW(None) }.unwrap();
  let name = {
    let mut filename: [u16; 64] = [0; 64];
    let length =
      unsafe { GetModuleFileNameW(Some(hinst), &mut filename[..]) };
    if length == 0 {
      eprintln!("error: {:?}", unsafe { GetLastError() });
      return;
    }
    String::from_utf16_lossy(&filename[..length as usize])
  };

  info!("module: {}", name);

  let monitors = enum_monitor_infos().unwrap();
  let mut handles = vec![];
  for monitor in monitors {
    let glazewm = GlazeWmService::start(monitor.dev_name.clone()).await;
    let minibar = Minibar::new(
      Config {
        bar_height: 30,
        pad_size: 5,
      },
      glazewm,
    );

    let handle = std::thread::spawn(move || {
      window::Window::<minibar::Minibar>::create(monitor, minibar)
        .start_message_loop();
    });
    handles.push(handle);
  }
  handles.into_iter().for_each(|x| x.join().unwrap());

  info!("done");
}

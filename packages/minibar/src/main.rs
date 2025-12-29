// #![windows_subsystem = "windows"]

use anyhow::Result;
use tracing::info;
use windows::Win32::{
  Foundation::GetLastError,
  System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW},
  UI::HiDpi::GetProcessDpiAwareness,
};

use crate::{
  logging::init_logger,
  minibar::{Config, Minibar},
  watcher::GlazeWmService,
};

// mod compositor;
mod color;
mod d2d;
mod logging;
mod minibar;
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
  let glazewm = GlazeWmService::start().await;
  let minibar = Minibar::new(
    Config {
      bar_height: 30,
      pad_size: 5,
    },
    glazewm,
  );

  let handle = std::thread::spawn(|| {
    window::Window::<minibar::Minibar>::create(minibar)
      .start_message_loop();
  });
  handle.join().unwrap();
  info!("done");
}

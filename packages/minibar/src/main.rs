#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{process, thread, time::Duration};

use tracing::info;
use windows::Win32::{
  Foundation::GetLastError,
  System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW},
  UI::HiDpi::GetProcessDpiAwareness,
};

use crate::{
  logging::init_logger,
  minibar::{Config, Minibar},
  monitor::enum_monitor_infos,
  wait::check_instance,
  watcher::GlazeWmService,
};

// mod compositor;
mod color;
mod d2d;
mod logging;
mod minibar;
mod monitor;
mod wait;
mod watcher;
mod window;

#[tokio::main]
async fn main() {
  init_logger();

  let _handle = check_instance()
    .inspect_err(|e| {
      eprintln!("check_instance: {:?}", e);
      process::exit(-1);
    })
    .unwrap();

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

  let mut cnt = 0;
  loop {
    cnt += 1;
    println!("generation: {}", cnt);
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
        let minibar =
          window::Window::<minibar::Minibar>::create(monitor, minibar)
            .start_message_loop();
        minibar.shutdown();
      });
      handles.push(handle);
    }
    handles.into_iter().for_each(|x| x.join().unwrap());

    thread::sleep(Duration::from_secs(1));
  }
}

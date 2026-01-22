use anyhow::Result;
use windows::{
  Win32::{
    self,
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE},
    System::Threading::CreateMutexW,
    UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW},
  },
  core::{Error, w},
};

pub fn check_instance() -> Result<HANDLE> {
  let mutex = unsafe {
    CreateMutexW(None, true, w!(r"Global\MinibarSingleInstanceLock"))
  };
  if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
    unsafe {
      MessageBoxW(
        None,
        w!("Another instance of Minibar is already running."),
        w!("Duplicated Minibar Instances"),
        MB_OK | MB_ICONERROR,
      );
    }
    Err(Error::from_thread().into())
  } else {
    Ok(mutex?)
  }
}

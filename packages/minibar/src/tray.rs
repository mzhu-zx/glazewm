use anyhow::{Context, Result};
use widestring::U16CString;
use windows::{
  Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    UI::{
      Shell::{
        NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE,
        NOTIFYICONDATAW, Shell_NotifyIconW,
      },
      WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos,
        IDI_APPLICATION, LoadIconW, MF_STRING, PostMessageW,
        SetForegroundWindow, TPM_RETURNCMD, TPM_RIGHTBUTTON,
        TrackPopupMenu, WM_APP, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP,
      },
    },
  },
  core::PCWSTR,
};

/// Message posted to the owning window when the tray icon is interacted
/// with. Uses the `WM_APP` range so it never collides with system messages.
pub const WM_TRAY_CALLBACK: u32 = WM_APP + 1;

/// Fixed identifier for the single minibar tray icon.
const TRAY_ICON_ID: u32 = 1;

/// Menu command identifier for the "Exit" entry.
const ID_MENU_EXIT: usize = 1;

/// A Windows notification-area ("tray") icon owned by a window.
///
/// Only one instance should exist at a time; it is removed automatically on
/// `Drop` and should also be removed when the owning window is destroyed.
pub struct TrayIcon {
  hwnd: HWND,
}

impl TrayIcon {
  /// Adds a tray icon owned by `hwnd`.
  ///
  /// Interactions are delivered to `hwnd` as `WM_TRAY_CALLBACK` messages,
  /// whose `lparam` low word holds the originating mouse message.
  pub fn add(hwnd: HWND, tooltip: &str) -> Result<Self> {
    // SAFETY: `IDI_APPLICATION` is a predefined system icon, so a `None`
    // instance handle is correct.
    let hicon = unsafe { LoadIconW(None, IDI_APPLICATION) }
      .context("Failed to load tray icon.")?;

    let mut data = NOTIFYICONDATAW {
      cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
      hWnd: hwnd,
      uID: TRAY_ICON_ID,
      uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
      uCallbackMessage: WM_TRAY_CALLBACK,
      hIcon: hicon,
      ..Default::default()
    };

    // Copy the tooltip into the fixed-size `szTip` buffer, leaving room for
    // the trailing null terminator.
    let tip = U16CString::from_str_truncate(tooltip);
    let tip = tip.as_slice_with_nul();
    let len = tip.len().min(data.szTip.len());
    data.szTip[..len].copy_from_slice(&tip[..len]);

    // SAFETY: `data` is fully initialized and outlives the call.
    let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) }.as_bool();
    if !added {
      anyhow::bail!("Shell_NotifyIconW(NIM_ADD) failed.");
    }

    Ok(Self { hwnd })
  }

  /// Handles a `WM_TRAY_CALLBACK` message.
  ///
  /// Shows the context menu on a left or right click and returns whether the
  /// user chose to exit the application.
  pub fn on_callback(&self, lparam: LPARAM) -> Result<bool> {
    let event = (lparam.0 & 0xffff) as u32;
    if event == WM_RBUTTONUP || event == WM_LBUTTONUP {
      return self.show_context_menu();
    }
    Ok(false)
  }

  /// Shows the tray context menu at the cursor and returns whether "Exit"
  /// was selected.
  fn show_context_menu(&self) -> Result<bool> {
    let mut pt = POINT::default();
    // SAFETY: `pt` is a valid, writable `POINT`.
    unsafe { GetCursorPos(&mut pt) }.context("GetCursorPos failed.")?;

    // SAFETY: creates an owned popup menu, destroyed before returning.
    let menu = unsafe { CreatePopupMenu() }.context("CreatePopupMenu")?;
    let exit_label = U16CString::from_str_truncate("Exit");
    // SAFETY: `menu` is valid and the label outlives the call.
    let result = unsafe {
      AppendMenuW(
        menu,
        MF_STRING,
        ID_MENU_EXIT,
        PCWSTR(exit_label.as_ptr()),
      )
    };

    // The window must be foreground for the menu to dismiss correctly when
    // the user clicks elsewhere.
    // SAFETY: `self.hwnd` is a valid window handle.
    let _ = unsafe { SetForegroundWindow(self.hwnd) };

    // SAFETY: `menu` is valid; `TPM_RETURNCMD` makes this return the chosen
    // command id instead of posting a `WM_COMMAND`.
    let chosen = unsafe {
      TrackPopupMenu(
        menu,
        TPM_RIGHTBUTTON | TPM_RETURNCMD,
        pt.x,
        pt.y,
        Some(0),
        self.hwnd,
        None,
      )
    };

    // Recommended workaround so the menu closes properly on next click.
    // SAFETY: `self.hwnd` is a valid window handle.
    let _ = unsafe {
      PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0))
    };

    // SAFETY: `menu` is still valid and owned here.
    let _ = unsafe { DestroyMenu(menu) };
    result.context("AppendMenuW failed.")?;

    Ok(chosen.0 as usize == ID_MENU_EXIT)
  }

  /// Removes the tray icon from the notification area.
  fn remove(&self) {
    let data = NOTIFYICONDATAW {
      cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
      hWnd: self.hwnd,
      uID: TRAY_ICON_ID,
      ..Default::default()
    };
    // SAFETY: `data` identifies the icon added in `add` and outlives the
    // call.
    unsafe {
      let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
  }
}

impl Drop for TrayIcon {
  fn drop(&mut self) {
    self.remove();
  }
}

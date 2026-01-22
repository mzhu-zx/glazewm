use std::{
  i32,
  sync::{Once, atomic::AtomicU64},
};

use anyhow::{Context, Result};
use tracing::{debug, info};
use windows::{
  Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::{
      Direct2D::Common::D2D_SIZE_U,
      Gdi::{BeginPaint, EndPaint, PAINTSTRUCT, ValidateRect},
    },
    System::{
      Com::CoInitialize, LibraryLoader::GetModuleHandleW,
      Threading::GetCurrentThreadId,
    },
    UI::{
      Shell::{
        ABE_TOP, ABM_NEW, ABM_QUERYPOS, ABM_REMOVE, ABM_SETPOS,
        APPBARDATA, SHAppBarMessage,
      },
      WindowsAndMessaging::{
        CREATESTRUCTW, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWL_STYLE, GWLP_USERDATA, GetMessageW, GetWindowLongPtrW, IDC_ARROW, LoadCursorA, MSG, MoveWindow, PostQuitMessage, RegisterClassW, SW_NORMAL, SetWindowLongPtrW, SetWindowLongW, ShowWindow, TranslateMessage, WINDOW_EX_STYLE, WM_CREATE, WM_DESTROY, WM_DISPLAYCHANGE, WM_LBUTTONDOWN, WM_NCCREATE, WM_PAINT, WM_SIZE, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP
      },
    },
  },
  core::{PCSTR, PCWSTR, w},
};
use windows_numerics::Matrix3x2;

use crate::{color, d2d::D2DHost, minibar::Minibar, monitor::MonitorInfo};

static REGISTER_MAIN_WINDOW_CLASS: Once = Once::new();

pub struct Window<State> {
  hwnd: HWND,
  state: State,
  d2d_host: Option<D2DHost>,
  monitor: MonitorInfo,
  appbar_magic: u32,
}

impl<T> Window<T> {
  /// Show the window and start the message pump.
  pub fn start_message_loop(self) -> T {
    let _ = unsafe { ShowWindow(self.hwnd, SW_NORMAL) };

    let mut msg: MSG = Default::default();
    loop {
      let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
      if ret <= 0 {
        if ret < 0 {
          use windows::core::Error;
          eprintln!("error in pump: {}", Error::from_thread().message());
        }
        println!("WM_QUIT!");
        break;
      };
      // println!("pump event: {}", msg.message);
      unsafe {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
      };
    }

    self.state
  }
}

// impl<State: std::fmt::Debug> Window<State> {
impl Window<Minibar> {
  /// Window message handler.
  extern "system" fn window_proc(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
  ) -> LRESULT {
    unsafe {
      let window_ptr = match umsg {
        WM_NCCREATE => {
          let create_struct = lparam.0 as *mut CREATESTRUCTW;
          let window = (*create_struct).lpCreateParams as *mut Self;
          (*window).hwnd = hwnd;
          let magic = create_appbar(hwnd, 30, (*window).monitor.rect)
            .context("make app bar")
            .unwrap();
          (*window).appbar_magic = magic;
          SetWindowLongPtrW(hwnd, GWLP_USERDATA, window as isize);
          window
        }
        WM_CREATE => {
          let window_ptr =
            GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self;
          let d2d_host =
            D2DHost::init(hwnd).context("intialize Direct2D").unwrap();
          (*window_ptr).d2d_host = Some(d2d_host);
          window_ptr
        }
        _ => GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Self,
      };

      if let Some(window) = window_ptr.as_mut() {
        match window.handle_message(umsg, wparam, lparam) {
          Ok(handled) => {
            if !handled {
              DefWindowProcW(hwnd, umsg, wparam, lparam)
            } else {
              LRESULT(0)
            }
          }
          Err(err) => {
            eprintln!("[{}] error in event loop: {}", umsg, err);
            LRESULT(-1)
          }
        }
      } else {
        DefWindowProcW(hwnd, umsg, wparam, lparam)
      }
    }
  }

  /// The user-defined handler
  fn handle_message(
    &mut self,
    umsg: u32,
    _wparam: WPARAM,
    lparam: LPARAM,
  ) -> Result<bool> {
    // debug!("{:#X}", umsg);
    match umsg {
      WM_CREATE => {
        self
          .state
          .on_create(unsafe { GetCurrentThreadId() }, self.hwnd)?;
        // self.state.on_create(unsafe { GetCurrentThreadId() })?;
      }
      WM_SIZE => {
        let (width, height) = lparam_as_xy(&lparam);
        unsafe {
          // avoid resizing when the D2d host is not ready
          // if let Ok(d) = D2DHost::init(self.hwnd) {
          if let Some(d) = self.d2d_host.as_ref() {
            let size = D2D_SIZE_U {
              width: width as _,
              height: height as _,
            };
            debug!("d2d resize: {:?}", size);
            d.render_target.Resize(&size)?;
          }
        };
      }
      WM_PAINT => {
        debug!("WM_PAINT!");
        let mut ps: PAINTSTRUCT = Default::default();
        let _hdc = unsafe { BeginPaint(self.hwnd, &mut ps) };
        if let Some(d2d_host) = self.d2d_host.as_ref() {
          let rt = &d2d_host.render_target;
          unsafe {
            rt.BeginDraw();
            rt.SetTransform(&Matrix3x2::identity());
            rt.Clear(Some(&color::D2D1_COLOR_WHITE));
          }
          self.state.on_paint_d2d(d2d_host)?;
          unsafe {
            rt.EndDraw(None, None)?;
          }
        }
        unsafe {
          let _ = ValidateRect(Some(self.hwnd), None);
          let _ = EndPaint(self.hwnd, &ps);
        }
      }
      WM_LBUTTONDOWN => {
        let (x, y) = lparam_as_xy(&lparam);
        debug!("click: ({}, {})", x, y);
        self.state.on_click(x, y)?;
      }
      WM_DISPLAYCHANGE => {
        unsafe { DestroyWindow(self.hwnd)? };
        // if let Some(new_monitor) = self.monitor.update_monitor_info() {
        //   unsafe {
        //     info!(
        //       "change display to {:?}",
        //       RECT {
        //         left: new_monitor.rect.left,
        //         top: new_monitor.rect.top,
        //         right: new_monitor.rect.right,
        //         bottom: new_monitor.rect.top
        //           + self.state.config.bar_height,
        //       }
        //     );
        //     MoveWindow(
        //       self.hwnd,
        //       new_monitor.rect.left,
        //       new_monitor.rect.top,
        //       new_monitor.rect.right - new_monitor.rect.left,
        //       self.state.config.bar_height,
        //       true,
        //     )?
        //   }
        // } else {
        //   info!("i should have destroy the window now ({:?})",
        // &self.hwnd); }
      }
      WM_DESTROY => {
        self.remove_appbar();
        unsafe { PostQuitMessage(0) };
      }
      _ => return Ok(false),
    }
    Ok(true)
  }

  // pub fn create(state: State) -> Box<Self> {
  /// Create a new main window.
  pub fn create(monitor: MonitorInfo, state: Minibar) -> Box<Self> {
    info!("monitor: {:?}", monitor);
    unsafe { CoInitialize(None) }.ok().unwrap();

    const CLASS_NAME: PCWSTR = w!("Minibar-Main");

    let hinst: HINSTANCE =
      unsafe { GetModuleHandleW(None) }.unwrap().into();
    REGISTER_MAIN_WINDOW_CLASS.call_once(|| {
      let wc = WNDCLASSW {
        lpfnWndProc: Some(Self::window_proc),
        hInstance: hinst,
        lpszClassName: CLASS_NAME,
        hCursor: unsafe { LoadCursorA(None, PCSTR(IDC_ARROW.0 as _)) }
          .unwrap(),
        ..Default::default()
      };
      unsafe { RegisterClassW(&wc) };
    });
    let rect = monitor.rect;
    let mut boxed = Box::new(Self {
      state,
      hwnd: Default::default(),
      d2d_host: None,
      monitor,
      appbar_magic: 0,
    });

    let hwnd = unsafe {
      CreateWindowExW(
        WS_EX_TOOLWINDOW,
        CLASS_NAME,
        w!("Minibar - main"),
        WS_POPUP,
        rect.left,
        rect.top,
        rect.right - rect.left,
        boxed.state.config.bar_height,
        None,
        None,
        Some(hinst),
        Some(boxed.as_mut() as *mut _ as _),
      )
    }
    .unwrap();
    unsafe {
      SetWindowLongW(hwnd, GWL_STYLE, (WS_POPUP).0 as i32);
    }
    // if let Err(e) = create_appbar(hwnd, boxed.state.config.bar_height) {
    //   eprintln!("error in appbar: {:?}", e);
    // }
    boxed
  }

  fn remove_appbar(&self) {
    unsafe {
      let mut data = APPBARDATA {
        cbSize: std::mem::size_of::<APPBARDATA>() as u32,
        hWnd: self.hwnd,
        uCallbackMessage: self.appbar_magic,
        ..Default::default()
      };
      SHAppBarMessage(ABM_REMOVE, &mut data);
    };
  }
}

static GLOBAL_COUNTER: AtomicU64 = AtomicU64::new(0);

const MINIBAR_MAGIC_NUMBER: u32 = 42;
fn create_appbar(
  app_hwnd: HWND,
  bar_height: i32,
  rect: RECT,
) -> Result<u32> {
  let magic = MINIBAR_MAGIC_NUMBER
    + GLOBAL_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
      as u32;
  let _ = unsafe {
    let mut data = APPBARDATA {
      cbSize: std::mem::size_of::<APPBARDATA>() as u32,
      hWnd: app_hwnd,
      uCallbackMessage: magic,
      ..Default::default()
    };
    SHAppBarMessage(ABM_NEW, &mut data)
  };

  let mut data = APPBARDATA {
    cbSize: std::mem::size_of::<APPBARDATA>() as u32,
    hWnd: app_hwnd,
    uCallbackMessage: magic,
    uEdge: ABE_TOP,
    rc: RECT {
      left: rect.left,
      top: rect.top,
      right: rect.right,
      bottom: bar_height,
    },
    ..Default::default()
  };
  unsafe {
    SHAppBarMessage(ABM_QUERYPOS, &mut data);
    SHAppBarMessage(ABM_SETPOS, &mut data);
    let rc = &data.rc;
    println!("appbar geometry: {:?}", rc);
    MoveWindow(
      app_hwnd,
      rc.left,
      rc.top,
      rc.right - rc.left,
      rc.bottom - rc.top,
      false,
    )?
  }
  Ok(magic)
}

/// LOWORD is X
/// HIWORD is Y
fn lparam_as_xy(lparam: &LPARAM) -> (u16, u16) {
  let u = lparam.0 as usize;
  ((u & 0xffff) as u16, ((u >> 16) & 0xfff) as u16)
}

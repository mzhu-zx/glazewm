use anyhow::{Context, Result};
use tracing::{debug, error, info};
use widestring::U16CString;
use windows::Win32::{
  Foundation::HWND,
  Graphics::{
    Direct2D::{Common::D2D_RECT_F, D2D1_DRAW_TEXT_OPTIONS_NONE},
    DirectWrite::DWRITE_TEXT_METRICS,
  },
  UI::WindowsAndMessaging::{PostMessageA, WM_PAINT},
};
use windows_numerics::Vector2;

use crate::{
  color,
  d2d::D2DHost,
  watcher::{GlazeWmService, WindowStatus, WorkspaceStatus},
};

pub struct Config {
  pub bar_height: i32,
  pub pad_size: i32,
}

#[derive(Debug)]
struct Clickable<T> {
  /// Location of the indicator
  pub location: D2D_RECT_F,
  pub status: T,
}

// pub status: WorkspaceStatus,

impl<T> Clickable<T> {
  fn hit_test(&self, pt: Vector2) -> bool {
    (self.location.left <= pt.X)
      && (pt.X <= self.location.right)
      && (self.location.top <= pt.Y)
      && (pt.Y <= self.location.bottom)
  }

  fn wrap_default(status: T) -> Self {
    Clickable {
      location: Default::default(),
      status,
    }
  }
}

type WorkspaceButton = Clickable<WorkspaceStatus>;
type WindowButton = Clickable<WindowStatus>;

pub struct Minibar {
  pub config: Config,
  ws_buttons: Vec<WorkspaceButton>,
  win_buttons: Vec<WindowButton>,
  glazewm: GlazeWmService,
}

impl Minibar {
  pub fn new(config: Config, glazewm: GlazeWmService) -> Self {
    let ws_buttons = glazewm
      .current_workspaces()
      .into_iter()
      .map(Clickable::wrap_default)
      .collect();
    let win_buttons = glazewm
      .current_windows()
      .into_iter()
      .map(Clickable::wrap_default)
      .collect();

    Self {
      config,
      ws_buttons,
      win_buttons,
      glazewm,
    }
  }

  fn update_state(&mut self) {
    self.ws_buttons = self
      .glazewm
      .current_workspaces()
      .into_iter()
      .map(Clickable::wrap_default)
      .collect();
    self.win_buttons = self
      .glazewm
      .current_windows()
      .into_iter()
      .map(Clickable::wrap_default)
      .collect();
    debug!("new ws: {:?}", self.ws_buttons);
    debug!("new wins: {:?}", self.win_buttons);
  }

  pub fn shutdown(self) {
    self.glazewm.stop()
  }
}

impl Minibar {
  pub fn on_paint_d2d(&mut self, d2d_host: &D2DHost) -> Result<()> {
    self.update_state();

    debug!("paint");

    let solid_brush = unsafe {
      d2d_host
        .render_target
        .CreateSolidColorBrush(&color::D2D1_COLOR_WHITE_SMOKE, None)
    }?;

    let rt_size = unsafe { d2d_host.render_target.GetSize() };
    debug!("render target size: {:?}", rt_size);
    unsafe {
      d2d_host.render_target.FillRectangle(
        &D2D_RECT_F {
          top: 0.,
          left: 0.,
          bottom: rt_size.height,
          right: rt_size.width,
        },
        &solid_brush,
      );
    }

    let mut x = self.config.bar_height as f32;

    // paint ws indicator L2R
    for indicator in self.ws_buttons.iter_mut() {
      // compute the layout
      let ws_name =
        U16CString::from_str_truncate(indicator.status.fullname());
      let layout = unsafe {
        d2d_host.dwrite_factory.CreateTextLayout(
          ws_name.as_ref(),
          &d2d_host.font,
          f32::MAX,
          f32::MAX,
        )
      }?;

      let metrics: DWRITE_TEXT_METRICS = {
        let mut it = Default::default();
        unsafe { layout.GetMetrics(&mut it) }?;
        it
      };

      let box_pad = 5.0;

      let centered = Vector2 {
        X: x + box_pad,
        Y: self.config.bar_height as f32 / 2.0 - metrics.height / 2.0,
      };

      let left = x;
      let top = self.config.pad_size as f32;

      let box_h =
        (self.config.bar_height - 2 * self.config.pad_size) as f32;
      let box_w = 2.0 * box_pad + metrics.widthIncludingTrailingWhitespace;
      let right = left + box_w;
      let bottom = top + box_h;
      x = right + self.config.pad_size as f32;

      indicator.location = {
        D2D_RECT_F {
          left,
          top,
          right,
          bottom,
        }
      };

      unsafe {
        if indicator.status.activated {
          solid_brush.SetColor(&color::D2D1_COLOR_AQUAMARINE);
        } else {
          solid_brush.SetColor(&color::D2D1_COLOR_LIGHT_GRAY);
        }

        d2d_host
          .render_target
          .FillRectangle(&indicator.location, &solid_brush);
        solid_brush.SetColor(&color::D2D1_COLOR_BLACK);

        // paint border
        d2d_host.render_target.DrawRectangle(
          &indicator.location,
          &solid_brush,
          1.0,
          None,
        );
        d2d_host.render_target.DrawTextLayout(
          centered,
          &layout,
          &solid_brush,
          D2D1_DRAW_TEXT_OPTIONS_NONE,
        );
      }
    }


    let mut x = rt_size.width - self.config.bar_height as f32;

    // paint window indicators RtoL
    for indicator in self.win_buttons.iter_mut().rev() {
      // compute the layout
      let ws_name =
        U16CString::from_str_truncate(&indicator.status.title);
      let layout = unsafe {
        d2d_host.dwrite_factory.CreateTextLayout(
          ws_name.as_ref(),
          &d2d_host.font,
          f32::MAX,
          f32::MAX,
        )
      }?;

      let metrics: DWRITE_TEXT_METRICS = {
        let mut it = Default::default();
        unsafe { layout.GetMetrics(&mut it) }?;
        it
      };

      let box_pad = 5.0;
      let box_w = 2.0 * box_pad + metrics.widthIncludingTrailingWhitespace;
      let left = x - box_w;
      let v_centered = Vector2 {
        X: x - box_w + box_pad,
        Y: self.config.bar_height as f32 / 2.0 - metrics.height / 2.0,
      };
      let top = self.config.pad_size as f32;
      let box_h =
        (self.config.bar_height - 2 * self.config.pad_size) as f32;

      let right = x;
      let bottom = top + box_h;

      x = left - self.config.pad_size as f32;

      indicator.location = {
        D2D_RECT_F {
          left,
          top,
          right,
          bottom,
        }
      };

      unsafe {
        if indicator.status.activated {
          solid_brush.SetColor(&color::D2D1_COLOR_AQUAMARINE);
        } else {
          solid_brush.SetColor(&color::D2D1_COLOR_LIGHT_GRAY);
        }

        d2d_host
          .render_target
          .FillRectangle(&indicator.location, &solid_brush);
        solid_brush.SetColor(&color::D2D1_COLOR_BLACK);

        // paint border
        d2d_host.render_target.DrawRectangle(
          &indicator.location,
          &solid_brush,
          1.0,
          None,
        );
        d2d_host.render_target.DrawTextLayout(
          v_centered,
          &layout,
          &solid_brush,
          D2D1_DRAW_TEXT_OPTIONS_NONE,
        );
      }
    }

    Ok(())
  }

  // implement click actions here
  pub fn on_click(&mut self, x: u16, y: u16) -> Result<()> {
    if let Some(found) = self.ws_buttons.iter().find(|i| {
      i.hit_test(Vector2 {
        X: x as _,
        Y: y as _,
      })
    }) {
      info!("hit! ({:?})", found);
      self.glazewm.send_command(format!(
        "command focus --workspace {}",
        found.status.name
      ))?;
      return Ok(());
    }

    if let Some(found) = self.win_buttons.iter().find(|i| {
      i.hit_test(Vector2 {
        X: x as _,
        Y: y as _,
      })
    }) {
      info!("hit! ({:?})", found);
      self.glazewm.send_command(format!(
        "command focus --container-id {}",
        found.status.id
      ))?;
      return Ok(());
    }


    Ok(())
  }

  // // implement hover effect here
  // pub fn on_move() {}

  pub fn on_create(
    &mut self,
    _tid: u32,
    hwnd: HWND, /* tid: u32 */
  ) -> Result<()> {
    let mut rx = self
      .glazewm
      .subscribe_repaint()
      .context("already subscribed")?;
    let hwnd_i = hwnd.0 as u64;
    std::thread::spawn(move || {
      loop {
        let hwnd = HWND(hwnd_i as _);
        if let Some(()) = rx.blocking_recv() {
          debug!("repaint request received.");
          unsafe {
            if let Err(e) = PostMessageA(
              Some(hwnd),
              WM_PAINT,
              Default::default(),
              Default::default(),
            ) {
              error!("{}", e)
            } else {
              debug!("workspace update! ({:?})", 1)
            }
          }
        }
      }
    });
    Ok(())
  }
}

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
  d2d::{self, D2DHost},
  watcher::{self, GlazeWmService, WorkspaceStatus},
};

pub struct Config {
  pub bar_height: i32,
  pub pad_size: i32,
}

#[derive(Debug)]
struct Indicator {
  /// Location of the indicator
  pub location: D2D_RECT_F,
  pub status: WorkspaceStatus,
}

impl Indicator {
  fn hit_test(&self, pt: Vector2) -> bool {
    (self.location.left <= pt.X)
      && (pt.X <= self.location.right)
      && (self.location.top <= pt.Y)
      && (pt.Y <= self.location.bottom)
  }
}

pub struct Minibar {
  pub config: Config,
  // pub form_factor: Rect,
  ws_indicators: Vec<Indicator>,
  glazewm: GlazeWmService,
}

impl Minibar {
  pub fn new(config: Config, glazewm: GlazeWmService) -> Self {
    let ws_indicators = glazewm
      .current_workspaces()
      .into_iter()
      .map(|status| Indicator {
        location: Default::default(),
        status,
      })
      .collect();
    let ret = Self {
      config,
      ws_indicators,
      glazewm,
    };
    ret
  }

  fn update_workspace(&mut self, d2d_host: &D2DHost) {
    let ws_indicators = self
      .glazewm
      .current_workspaces()
      .into_iter()
      .map(|status| Indicator {
        location: Default::default(),
        status,
      })
      .collect();
    self.ws_indicators = ws_indicators;
    debug!("new indicators: {:?}", self.ws_indicators);
  }
}

impl Minibar {
  pub fn on_paint_d2d(&mut self, d2d_host: &D2DHost) -> Result<()> {
    self.update_workspace(d2d_host);

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

    for indicator in self.ws_indicators.iter_mut() {
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

    Ok(())
  }

  // implement click actions here
  pub fn on_click(&mut self, x: u16, y: u16) -> Result<()> {
    if let Some(found) = self.ws_indicators.iter().find(|i| {
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
    }
    Ok(())
  }

  // implement hover effect here
  pub fn on_move() {}

  pub fn on_create(
    &mut self,
    tid: u32,
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

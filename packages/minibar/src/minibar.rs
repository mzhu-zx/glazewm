use anyhow::{Context, Result};
use tracing::{debug, error, info};
use windows::Win32::{
  Foundation::HWND,
  Graphics::{
    Direct2D::Common::D2D_RECT_F,
    Gdi::{InvalidateRect, RDW_INVALIDATE, RedrawWindow, UpdateWindow},
  },
  UI::WindowsAndMessaging::{
    HWND_BROADCAST, PostMessageA, PostThreadMessageA, WM_PAINT,
  },
};
use windows_numerics::Vector2;

use crate::{
  color,
  d2d::D2DHost,
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
    let mut ret = Self {
      config,
      ws_indicators,
      glazewm,
    };
    ret.place_rects();
    ret
  }

  fn update_workspace(&mut self) {
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
    self.place_rects();
    debug!("new indicators: {:?}", self.ws_indicators);
  }
}

impl Minibar {
  pub fn on_paint_d2d(&mut self, d2d_host: &D2DHost) -> Result<()> {
    self.update_workspace();

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

    for indicator in self.ws_indicators.iter() {
      unsafe {
        if indicator.status.activated {
          solid_brush.SetColor(&color::D2D1_COLOR_AQUAMARINE);
        } else {
          solid_brush.SetColor(&color::D2D1_COLOR_GRAY);
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

  fn place_rects(&mut self) {
    for (index, indicator) in self.ws_indicators.iter_mut().enumerate() {
      indicator.location = {
        let box_size =
          (self.config.bar_height - 2 * self.config.pad_size) as f32;

        let left = self.config.pad_size as f32
          + (box_size + self.config.pad_size as f32) * (1 + index) as f32;
        let top = self.config.pad_size as f32;
        let right = left + box_size;
        let bottom = top + box_size;

        D2D_RECT_F {
          left,
          top,
          right,
          bottom,
        }
      };
    }
    debug!(
      "locs: {:?}",
      self
        .ws_indicators
        .iter()
        .map(|i| i.location)
        .collect::<Vec<D2D_RECT_F>>()
    )
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

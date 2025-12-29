use anyhow::{Context, Result, anyhow, ensure};
use tracing::{debug, info};
use windows::{
  Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Direct2D::{
      Common::D2D_SIZE_U, D2D1_FACTORY_TYPE_SINGLE_THREADED,
      D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_PROPERTIES,
      D2D1_RENDER_TARGET_TYPE_SOFTWARE, D2D1CreateFactory, ID2D1Factory,
      ID2D1HwndRenderTarget, ID2D1RenderTarget,
    },
    UI::WindowsAndMessaging::{GetClientRect, GetWindowRect},
  },
  core::Interface,
};

pub struct D2DHost {
  pub d2d_factory: ID2D1Factory,
  pub render_target: ID2D1HwndRenderTarget,
}

impl D2DHost {
  pub fn init(hwnd: HWND) -> Result<D2DHost> {
    let d2d_factory: ID2D1Factory =
      unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None) }?;

    let draw_rect: RECT = {
      let mut rc = RECT::default();
      unsafe {
        GetWindowRect(hwnd, &mut rc)?;
      }
      debug!("d2d on rect: {:?}", rc);
      rc
    };

    // [render_target] for drawing and brushes
    let render_target: ID2D1HwndRenderTarget = unsafe {
      let rtp = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE, // disable HW acceleration,
        ..Default::default()
      };

      let hwnd_rtp = D2D1_HWND_RENDER_TARGET_PROPERTIES {
        hwnd,
        pixelSize: D2D_SIZE_U {
          width: (draw_rect.right - draw_rect.left) as u32,
          height: (draw_rect.bottom - draw_rect.top) as u32,
        },
        ..Default::default()
      };
      d2d_factory.CreateHwndRenderTarget(&rtp, &hwnd_rtp)
    }?;
    unsafe {
      // disable scaling to use the device pixels 
      render_target.SetDpi(96.0, 96.0);
    }
    Ok(D2DHost {
      d2d_factory,
      render_target,
    })
  }
}

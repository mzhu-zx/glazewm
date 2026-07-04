use anyhow::Result;
use tracing::debug;
use windows::
  Win32::{
    Foundation::{HWND, RECT},
    Graphics::{
      Direct2D::{
        Common::D2D_SIZE_U, D2D1_FACTORY_TYPE_SINGLE_THREADED,
        D2D1_HWND_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_PROPERTIES,
        D2D1_RENDER_TARGET_TYPE_SOFTWARE, D2D1CreateFactory, ID2D1Factory,
        ID2D1HwndRenderTarget,
      },
      DirectWrite::{
        DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL,
        DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
        DWRITE_PARAGRAPH_ALIGNMENT_NEAR, DWRITE_TEXT_ALIGNMENT_LEADING,
        DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory,
        IDWriteFontCollection, IDWriteTextFormat,
      },
    },
    UI::WindowsAndMessaging::GetWindowRect,
  }
;

pub struct D2DHost {
  pub d2d_factory: ID2D1Factory,
  pub dwrite_factory: IDWriteFactory,
  pub render_target: ID2D1HwndRenderTarget,
  pub font: IDWriteTextFormat,
}

impl D2DHost {
  /// Initialize a Direct2D host bound to `hwnd`.
  ///
  /// `dpi` is the effective DPI of the target monitor (`96` is 100%
  /// scaling). The render target is configured with this DPI so that
  /// drawing done in design units (DIPs) is scaled to physical device
  /// pixels.
  pub fn init(hwnd: HWND, dpi: f32) -> Result<D2DHost> {
    let d2d_factory: ID2D1Factory = unsafe {
      D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)
    }?;

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
        r#type: D2D1_RENDER_TARGET_TYPE_SOFTWARE, /* disable HW
                                                   * acceleration, */
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
      // Match the render target DPI to the monitor so that design-unit
      // (DIP) coordinates are scaled up to physical device pixels.
      render_target.SetDpi(dpi, dpi);
    }

    let dwrite_factory =
      unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED) }?;
    let font = create_jetbrains_mono_12(&dwrite_factory)?;
    Ok(D2DHost {
      dwrite_factory,
      d2d_factory,
      render_target,
      font,
    })
  }
}

pub fn create_jetbrains_mono_12(
  dwrite: &IDWriteFactory,
) -> Result<IDWriteTextFormat> {
  let mut sys_fonts: Option<IDWriteFontCollection> = None;
  unsafe { dwrite.GetSystemFontCollection(&mut sys_fonts, false) }?;
  use windows::core::w;

  let jb = w!("JetBrains Mono");
  let mut index: u32 = 0;
  let mut exists = Default::default();
  unsafe {
    sys_fonts
      .unwrap()
      .FindFamilyName(jb, &mut index, &mut exists)?
  };

  let family = if exists.as_bool() {
    w!("JetBrains Mono")
  } else {
    w!("Consolas")
  };
  let fmt: IDWriteTextFormat = unsafe {
    dwrite.CreateTextFormat(
      family,
      None, // system collection
      DWRITE_FONT_WEIGHT_NORMAL,
      DWRITE_FONT_STYLE_NORMAL,
      DWRITE_FONT_STRETCH_NORMAL,
      12.0, // "12pt" as commonly used in DirectWrite; value is in DIPs
      w!("en-us"),
    )?
  };

  unsafe {
    fmt.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
    fmt.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_NEAR)?;
    fmt.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
  }

  Ok(fmt)
}

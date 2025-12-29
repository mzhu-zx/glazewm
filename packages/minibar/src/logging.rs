use std::sync::Once;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

static INIT: Once = Once::new();

pub fn init_logger() {
  INIT.call_once(|| {
    tracing_subscriber::registry()
      .with(fmt::layer())
      .with(EnvFilter::from_default_env())
      .init();
  });
}

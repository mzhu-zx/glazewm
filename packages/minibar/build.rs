use embed_manifest::{embed_manifest, new_manifest};

fn main() {
  let manifest = new_manifest("minibar")
    .dpi_awareness(embed_manifest::manifest::DpiAwareness::PerMonitorV2);
  embed_manifest(manifest).expect("unable to embed manifest file");
  println!("cargo:rerun-if-changed=build.rs");
}

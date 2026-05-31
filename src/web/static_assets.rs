#![cfg(feature = "vrunner")]
#![allow(dead_code, unused_imports)]
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/admin/"]
pub struct AdminAssets;

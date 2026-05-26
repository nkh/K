use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "static/admin/"]
pub struct AdminAssets;

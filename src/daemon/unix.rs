use anyhow::Result;
use daemonize::Daemonize;
use std::fs::OpenOptions;

use crate::config::schema::Config;

pub fn daemonize(cfg: &Config) -> Result<()> {
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.daemon.stdout_file)?;
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&cfg.daemon.stderr_file)?;

    let daemonize = Daemonize::new()
        .working_directory("/tmp")
        .stdout(stdout)
        .stderr(stderr);

    daemonize.start()?;
    Ok(())
}

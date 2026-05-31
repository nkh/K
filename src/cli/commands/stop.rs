use anyhow::Result;

use crate::instance::info::InstanceInfo;

/// Stop a vrunner instance by sending SIGTERM to its PID.
///
/// If no PID is given, stops the only instance if exactly one is running.
pub fn handle_stop_command(pid: Option<u32>, instances: &[InstanceInfo]) -> Result<()> {
    let target_pid = resolve_stop_target(pid, instances);

    #[cfg(unix)]
    {
        let ret = unsafe { libc::kill(target_pid as i32, libc::SIGTERM) };
        if ret != 0 {
            let errno = std::io::Error::last_os_error();
            anyhow::bail!(
                "Failed to stop instance {} (PID {}): {}",
                target_pid, target_pid, errno
            );
        }
        println!("Sent SIGTERM to vrunner instance (PID {})", target_pid);
    }
    #[cfg(not(unix))]
    {
        // Fallback: use std::process::Command to send kill signal
        std::process::Command::new("kill")
            .arg(target_pid.to_string())
            .spawn()?
            .wait()?;
        println!("Sent kill signal to vrunner instance (PID {})", target_pid);
    }

    Ok(())
}

/// Resolve the target PID for the `vrunner stop` subcommand.
/// If no PID is given, resolves to the only instance if exactly one is running.
pub fn resolve_stop_target(pid: Option<u32>, instances: &[InstanceInfo]) -> u32 {
    match pid {
        Some(p) => p,
        None => match instances.len() {
            0 => {
                eprintln!("No vrunner instances running.");
                std::process::exit(1);
            }
            1 => {
                let p = instances[0].pid;
                println!("Stopping only running instance (PID {})", p);
                p
            }
            _ => {
                eprintln!("Multiple vrunner instances running. Specify which one to stop:");
                for inst in instances {
                    eprintln!("  PID {} -- {}", inst.pid, inst.command.as_deref().unwrap_or("(idle)"));
                }
                eprintln!("Usage: vrunner stop <PID>");
                std::process::exit(1);
            }
        },
    }
}

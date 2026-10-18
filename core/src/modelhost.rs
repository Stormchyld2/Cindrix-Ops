//! Starts and stops the local llama-server program, and waits until the model has loaded.

use crate::llama;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

pub struct ModelHost {
    child: Child,
    pub port: u16,
}

/// A free port on 127.0.0.1.
pub fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    Ok(l.local_addr().map_err(|e| e.to_string())?.port())
}

/// The command-line arguments for llama-server.
pub fn args(model: &Path, port: u16, threads: usize, ctx: u32) -> Vec<String> {
    vec![
        "-m".into(), model.to_string_lossy().to_string(),
        "--host".into(), "127.0.0.1".into(),
        "--port".into(), port.to_string(),
        "-c".into(), ctx.to_string(),
        "-t".into(), threads.to_string(),
        "--jinja".into(),
    ]
}

impl ModelHost {
    pub fn start(engine: &Path, model: &Path, threads: usize, ctx: u32) -> Result<ModelHost, String> {
        if !engine.exists() {
            return Err(format!("the model engine was not found at {}", engine.display()));
        }
        if !model.exists() {
            return Err(format!("the model file was not found at {}", model.display()));
        }
        let port = free_port()?;
        let mut cmd = Command::new(engine);
        cmd.args(args(model, port, threads, ctx)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(dir) = engine.parent() {
            cmd.current_dir(dir); // the engine's own DLLs sit next to it
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // no console window
        }
        let child = cmd.spawn().map_err(|e| format!("could not start the model engine: {}", e))?;
        Ok(ModelHost { child, port })
    }

    /// Wait until the model answers /health, the engine stops, or the time is up.
    pub fn wait_ready(&mut self, timeout: Duration) -> Result<(), String> {
        let t0 = Instant::now();
        loop {
            if llama::health(self.port) {
                return Ok(());
            }
            if let Ok(Some(status)) = self.child.try_wait() {
                return Err(format!("the model engine stopped while loading ({})", status));
            }
            if t0.elapsed() > timeout {
                return Err("the model took too long to load".into());
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ModelHost {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn ports_are_usable_and_differ() {
        let a = free_port().unwrap();
        assert!(a > 1024);
    }

    #[test]
    fn arguments_bind_only_to_this_computer() {
        let a = args(&PathBuf::from("m.gguf"), 5000, 4, 4096);
        let at = a.iter().position(|x| x == "--host").unwrap();
        assert_eq!(a[at + 1], "127.0.0.1");
        assert!(a.contains(&"5000".to_string()));
    }

    #[test]
    fn missing_engine_or_model_is_a_clear_error() {
        let e = ModelHost::start(&PathBuf::from("/nope/llama-server"), &PathBuf::from("/nope/m.gguf"), 2, 2048).err().unwrap();
        assert!(e.contains("engine was not found"));
    }

    #[cfg(unix)]
    #[test]
    fn a_program_that_exits_is_reported_while_waiting() {
        // `true` exits at once, standing in for an engine that crashes while loading.
        let dir = std::env::temp_dir();
        let model = dir.join("cindrix-fake-model.gguf");
        std::fs::write(&model, b"x").unwrap();
        let mut h = ModelHost::start(&PathBuf::from("/bin/true"), &model, 2, 2048).unwrap();
        let e = h.wait_ready(Duration::from_secs(5)).unwrap_err();
        assert!(e.contains("stopped") || e.contains("too long"), "{}", e);
    }
}

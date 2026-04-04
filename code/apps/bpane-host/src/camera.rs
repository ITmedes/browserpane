//! Virtual camera input: receives H.264 access units from the browser and feeds them
//! into a V4L2 loopback device so Chromium inside the container can expose a
//! webcam to sites via getUserMedia().

/// Camera input state - graceful degradation if no virtual V4L2 device exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraState {
    Available(String),
    Unavailable(String),
}

pub fn detect_camera() -> CameraState {
    #[cfg(target_os = "linux")]
    {
        let device = camera_device_from_env();
        if std::path::Path::new(&device).exists() {
            CameraState::Available(device)
        } else {
            CameraState::Unavailable(format!(
                "camera device {device} is not available; map a v4l2loopback device into the container"
            ))
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        CameraState::Unavailable("Camera input only available on Linux".to_string())
    }
}

#[cfg(target_os = "linux")]
pub struct CameraInput {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    device: String,
}

#[cfg(target_os = "linux")]
impl CameraInput {
    pub fn new() -> anyhow::Result<Self> {
        use std::process::{Command, Stdio};

        let device = match detect_camera() {
            CameraState::Available(device) => device,
            CameraState::Unavailable(reason) => anyhow::bail!(reason),
        };

        let fps = std::env::var("BPANE_CAMERA_FPS")
            .ok()
            .and_then(|raw| raw.parse::<u32>().ok())
            .unwrap_or(10)
            .clamp(1, 30);

        let mut child = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-fflags",
                "nobuffer",
                "-flags",
                "low_delay",
                "-f",
                "h264",
                "-i",
                "pipe:0",
                "-pix_fmt",
                "yuv420p",
                "-r",
                &fps.to_string(),
                "-f",
                "v4l2",
                &device,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("ffmpeg camera pipeline: no stdin"))?;

        tracing::info!(
            device = %device,
            fps,
            "camera: ffmpeg virtual camera pipeline started"
        );

        Ok(Self {
            child,
            stdin,
            device,
        })
    }

    pub fn write_frame(&mut self, access_unit: &[u8]) -> anyhow::Result<()> {
        use std::io::Write;

        self.stdin.write_all(access_unit)?;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for CameraInput {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        tracing::info!(device = %self.device, "camera: virtual camera stopped");
    }
}

#[cfg(target_os = "linux")]
fn camera_device_from_env() -> String {
    std::env::var("BPANE_CAMERA_DEVICE").unwrap_or_else(|_| "/dev/video0".to_string())
}

#[cfg(not(target_os = "linux"))]
pub struct CameraInput;

#[cfg(not(target_os = "linux"))]
impl CameraInput {
    pub fn new() -> anyhow::Result<Self> {
        anyhow::bail!("camera input is only available on Linux")
    }

    pub fn write_frame(&mut self, _access_unit: &[u8]) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "linux")]
    #[test]
    fn camera_device_from_env_defaults_to_video0() {
        unsafe {
            std::env::remove_var("BPANE_CAMERA_DEVICE");
        }
        assert_eq!(super::camera_device_from_env(), "/dev/video0");
    }
}

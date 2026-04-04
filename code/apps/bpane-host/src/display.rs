/// Virtual display setup.
///
/// Manages the virtual display backend (KMS virtual connector, Xvfb, or existing X11).

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    /// Use an existing X11 display.
    X11 { display: String },
    /// Launch Xvfb for development.
    Xvfb { display: String },
    /// Headless KMS (primary target for production).
    Kms { device: String },
}

impl DisplayMode {
    pub fn display_string(&self) -> &str {
        match self {
            Self::X11 { display } | Self::Xvfb { display } => display,
            Self::Kms { device } => device,
        }
    }
}

/// Detect the best available display mode.
pub fn detect_display_mode() -> DisplayMode {
    // Check for DISPLAY environment variable
    if let Ok(display) = std::env::var("DISPLAY") {
        return DisplayMode::X11 { display };
    }

    // Check for DRM device
    #[cfg(target_os = "linux")]
    {
        if std::path::Path::new("/dev/dri/card0").exists() {
            return DisplayMode::Kms {
                device: "/dev/dri/card0".to_string(),
            };
        }
    }

    // Fallback: suggest Xvfb
    DisplayMode::Xvfb {
        display: ":99".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_mode_display_string() {
        let x11 = DisplayMode::X11 {
            display: ":0".to_string(),
        };
        assert_eq!(x11.display_string(), ":0");

        let xvfb = DisplayMode::Xvfb {
            display: ":99".to_string(),
        };
        assert_eq!(xvfb.display_string(), ":99");
    }

    #[test]
    fn detect_returns_valid_mode() {
        let mode = detect_display_mode();
        match mode {
            DisplayMode::X11 { display } => assert!(!display.is_empty()),
            DisplayMode::Xvfb { display } => assert_eq!(display, ":99"),
            DisplayMode::Kms { device } => assert!(!device.is_empty()),
        }
    }
}

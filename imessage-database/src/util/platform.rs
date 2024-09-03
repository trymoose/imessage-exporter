/*!
 Contains data structures used to describe database platforms.
*/

use crate::tables::table::{DEFAULT_PATH_IOS, DEFAULT_PATH_IOS_FS_NEW, DEFAULT_PATH_IOS_FS_OLD};
use std::{fmt::Display, path::Path};

/// Represents the platform that created the database this library connects to
#[derive(PartialEq, Eq, Debug)]
pub enum Platform {
    /// macOS-sourced data
    #[allow(non_camel_case_types)]
    macOS,
    /// iOS-sourced data
    #[allow(non_camel_case_types)]
    iOS,
    /// extracted ios backup
    #[allow(non_camel_case_types)]
    iOSFS,
}

impl Platform {
    /// Try to determine the current platform, defaulting to macOS.
    pub fn determine(db_path: &Path) -> Self {
        if db_path.join(DEFAULT_PATH_IOS).exists() {
            return Self::iOS;
        }else if db_path.join(DEFAULT_PATH_IOS_FS_OLD).exists() || db_path.join(DEFAULT_PATH_IOS_FS_NEW).exists() {
            return Self::iOSFS;
        } else if db_path.is_file() {
            return Self::macOS;
        }
        // If we get here, the database is missing; that error is handled in the connection lifecycle
        Self::default()
    }

    /// Given user's input, return a variant if the input matches one
    pub fn from_cli(platform: &str) -> Option<Self> {
        match platform.to_lowercase().as_str() {
            "macos" => Some(Self::macOS),
            "ios" => Some(Self::iOS),
            "iosfs" => Some(Self::iOSFS),
            _ => None,
        }
    }
}

impl Default for Platform {
    /// The default Platform is [`Platform::macOS`].
    fn default() -> Self {
        Self::macOS
    }
}

impl Display for Platform {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::macOS => write!(fmt, "macOS"),
            Platform::iOS => write!(fmt, "iOS"),
            Platform::iOSFS => write!(fmt, "iOS_fs"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::util::platform::Platform;

    #[test]
    fn can_parse_macos_any_case() {
        assert!(matches!(Platform::from_cli("macos"), Some(Platform::macOS)));
        assert!(matches!(Platform::from_cli("MACOS"), Some(Platform::macOS)));
        assert!(matches!(Platform::from_cli("MacOS"), Some(Platform::macOS)));
    }

    #[test]
    fn can_parse_ios_any_case() {
        assert!(matches!(Platform::from_cli("ios"), Some(Platform::iO)));
        assert!(matches!(Platform::from_cli("IOS"), Some(Platform::iOS)));
        assert!(matches!(Platform::from_cli("iOS"), Some(Platform::iOS)));
    }

    #[test]
    fn can_parse_ios_fs_any_case() {
        assert!(matches!(Platform::from_cli("iosfs"), Some(Platform::iOSFS)));
        assert!(matches!(Platform::from_cli("iOSfs"), Some(Platform::iOSFS)));
        assert!(matches!(Platform::from_cli("iOSFS"), Some(Platform::iOSFS)));
        assert!(matches!(Platform::from_cli("iosFS"), Some(Platform::iOSFS)));
    }

    #[test]
    fn cant_parse_invalid() {
        assert!(Platform::from_cli("mac").is_none());
        assert!(Platform::from_cli("iphone").is_none());
        assert!(Platform::from_cli("").is_none());
    }
}

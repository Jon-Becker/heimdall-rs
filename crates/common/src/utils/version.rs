use std::fmt::Display;

use eyre::Result;

use super::http::get_json_from_url;

#[derive(Debug)]
/// Represents a semantic version number.
///
/// This struct follows the semantic versioning format of MAJOR.MINOR.PATCH,
/// with an optional release channel (e.g., alpha, beta).
pub struct Version {
    /// The major version number. Incremented for incompatible API changes.
    pub major: u32,
    /// The minor version number. Incremented for backward-compatible new functionality.
    pub minor: u32,
    /// The patch version number. Incremented for backward-compatible bug fixes.
    pub patch: u32,
    /// The optional release channel (e.g., "alpha", "beta", "rc").
    pub channel: Option<String>,
}

/// get the current version from cargo
pub fn current_version() -> Version {
    // get the current version from the cargo package
    let version_string = env!("CARGO_PKG_VERSION");

    // remove +<channel>... from the version string
    let version_channel =
        version_string.split('+').collect::<Vec<&str>>().get(1).map(|s| s.to_string());
    let version_string = version_string.split('+').collect::<Vec<&str>>()[0];
    let version_parts = version_string.split('.').collect::<Vec<&str>>();

    Version {
        major: version_parts[0].parse::<u32>().unwrap_or(0),
        minor: version_parts[1].parse::<u32>().unwrap_or(0),
        patch: version_parts[2].parse::<u32>().unwrap_or(0),
        channel: version_channel,
    }
}

/// get the latest version from github
pub async fn remote_version() -> Result<Version> {
    // get the latest release from github
    let remote_repository_url =
        "https://api.github.com/repos/Jon-Becker/heimdall-rs/releases/latest";

    // retrieve the latest release tag from github
    if let Some(release) = get_json_from_url(remote_repository_url, 1).await? {
        if let Some(tag_name) = release["tag_name"].as_str() {
            let version_string = tag_name.replace('v', "");
            let version_parts: Vec<&str> = version_string.split('.').collect();

            if version_parts.len() == 3 {
                let major = version_parts[0].parse::<u32>().unwrap_or(0);
                let minor = version_parts[1].parse::<u32>().unwrap_or(0);
                let patch = version_parts[2].parse::<u32>().unwrap_or(0);

                return Ok(Version { major, minor, patch, channel: None });
            }
        }
    }

    // if we can't get the latest release, return a default version
    Ok(Version { major: 0, minor: 0, patch: 0, channel: None })
}

/// get the latest nightly version from github
pub async fn remote_nightly_version() -> Result<Version> {
    // get the latest commit to main from github
    let remote_repository_url = "https://api.github.com/repos/Jon-Becker/heimdall-rs/commits/main";

    // get the latest release
    let mut remote_ver = remote_version().await?;

    // retrieve the latest commit from github
    if let Some(commit) = get_json_from_url(remote_repository_url, 1).await? {
        // get the latest commit hash
        if let Some(sha) = commit["sha"].as_str() {
            // channel is nightly.1234567
            remote_ver.channel = format!("nightly.{}", &sha[..7]).into();
        }
    }

    Ok(remote_ver)
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version_string = format!("{}.{}.{}{}", self.major, self.minor, self.patch, {
            if let Some(channel) = &self.channel {
                format!("+{channel}")
            } else {
                "".to_string()
            }
        });
        write!(f, "{version_string}")
    }
}

impl Version {
    /// greater than
    pub fn gt(&self, other: &Version) -> bool {
        self.major > other.major ||
            (self.major == other.major && self.minor > other.minor) ||
            (self.major == other.major && self.minor == other.minor && self.patch > other.patch)
    }

    /// greater than or equal to
    pub fn gte(&self, other: &Version) -> bool {
        self.major > other.major ||
            (self.major == other.major && self.minor > other.minor) ||
            (self.major == other.major && self.minor == other.minor && self.patch >= other.patch)
    }

    /// less than
    pub fn lt(&self, other: &Version) -> bool {
        self.major < other.major ||
            (self.major == other.major && self.minor < other.minor) ||
            (self.major == other.major && self.minor == other.minor && self.patch < other.patch)
    }

    /// less than or equal to
    pub fn lte(&self, other: &Version) -> bool {
        self.major < other.major ||
            (self.major == other.major && self.minor < other.minor) ||
            (self.major == other.major && self.minor == other.minor && self.patch <= other.patch)
    }

    /// Checks if this version is equal to another version.
    ///
    /// # Arguments
    ///
    /// * `other` - The version to compare with
    ///
    /// # Returns
    ///
    /// * `bool` - `true` if the versions are equal, `false` otherwise
    #[allow(clippy::should_implement_trait)]
    pub fn eq(&self, other: &Version) -> bool {
        self.major == other.major &&
            self.minor == other.minor &&
            self.patch == other.patch &&
            self.channel == other.channel
    }

    /// not equal to
    pub fn ne(&self, other: &Version) -> bool {
        self.major != other.major ||
            self.minor != other.minor ||
            self.patch != other.patch ||
            self.channel != other.channel
    }

    /// if the version is a nightly version
    pub fn is_nightly(&self) -> bool {
        self.channel.is_some() && self.channel.as_ref().unwrap().starts_with("nightly.")
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::version::*;

    #[test]
    fn test_greater_than() {
        let v1 = Version { major: 2, minor: 3, patch: 4, channel: None };
        let v2 = Version { major: 2, minor: 3, patch: 3, channel: None };
        let v3 = Version { major: 2, minor: 2, patch: 5, channel: None };
        let v4 = Version { major: 1, minor: 4, patch: 4, channel: None };

        assert!(v1.gt(&v2));
        assert!(v1.gt(&v3));
        assert!(v1.gt(&v4));
        assert!(!v2.gt(&v1));
        assert!(!v1.gt(&v1));
    }

    #[test]
    fn test_greater_than_or_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4, channel: None };
        let v2 = Version { major: 2, minor: 3, patch: 4, channel: None };

        assert!(v1.gte(&v2));
        assert!(v2.gte(&v1));
        assert!(v1.gte(&Version { major: 1, minor: 0, patch: 0, channel: None }));
    }

    #[test]
    fn test_less_than() {
        let v1 = Version { major: 2, minor: 3, patch: 4, channel: None };
        let v2 = Version { major: 2, minor: 3, patch: 5, channel: None };
        let v3 = Version { major: 2, minor: 4, patch: 4, channel: None };
        let v4 = Version { major: 3, minor: 3, patch: 4, channel: None };

        assert!(v1.lt(&v2));
        assert!(v1.lt(&v3));
        assert!(v1.lt(&v4));
        assert!(!v2.lt(&v1));
        assert!(!v1.lt(&v1));
    }

    #[test]
    fn test_less_than_or_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4, channel: None };
        let v2 = Version { major: 2, minor: 3, patch: 4, channel: None };

        assert!(v1.lte(&v2));
        assert!(v2.lte(&v1));
        assert!(v1.lte(&Version { major: 3, minor: 0, patch: 0, channel: None }));
    }

    #[test]
    fn test_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4, channel: None };
        let v2 = Version { major: 2, minor: 3, patch: 4, channel: None };
        let v3 = Version { major: 2, minor: 3, patch: 5, channel: None };

        assert!(v1.eq(&v2));
        assert!(!v1.eq(&v3));
    }

    #[test]
    fn test_not_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4, channel: None };
        let v2 = Version { major: 2, minor: 3, patch: 5, channel: None };
        let v3 = Version { major: 3, minor: 3, patch: 4, channel: None };

        assert!(v1.ne(&v2));
        assert!(v1.ne(&v3));
        assert!(!v1.ne(&Version { major: 2, minor: 3, patch: 4, channel: None }));
    }

    #[test]
    fn test_version_display() {
        let version = Version { major: 2, minor: 3, patch: 4, channel: None };

        assert_eq!(version.to_string(), "2.3.4");
    }

    #[test]
    fn test_version_current() {
        let version = current_version();

        assert_eq!(version.to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_version_remote() {
        let version = remote_version().await;

        assert!(version.is_ok());
    }

    #[tokio::test]
    async fn test_version_remote_nightly() {
        let version = remote_nightly_version().await;

        assert!(version.is_ok());
    }
}

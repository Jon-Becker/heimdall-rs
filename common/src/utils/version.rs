use std::fmt::Display;

use super::http::get_json_from_url;

#[derive(Debug)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

/// get the current version from cargo
pub fn current_version() -> Version {
    // get the current version from the cargo package
    let version_string = env!("CARGO_PKG_VERSION");

    // remove +<channel>... from the version string
    let version_string = version_string.split('+').collect::<Vec<&str>>()[0];

    let version_parts: Vec<&str> = version_string.split('.').collect();

    Version {
        major: version_parts[0].parse::<u32>().unwrap_or(0),
        minor: version_parts[1].parse::<u32>().unwrap_or(0),
        patch: version_parts[2].parse::<u32>().unwrap_or(0),
    }
}

/// get the latest version from github
pub async fn remote_version() -> Version {
    // get the latest release from github
    let remote_repository_url =
        "https://api.github.com/repos/Jon-Becker/heimdall-rs/releases/latest";

    // retrieve the latest release tag from github
    if let Some(release) = get_json_from_url(remote_repository_url, 1).await.unwrap() {
        if let Some(tag_name) = release["tag_name"].as_str() {
            let version_string = tag_name.replace('v', "");
            let version_parts: Vec<&str> = version_string.split('.').collect();

            if version_parts.len() == 3 {
                let major = version_parts[0].parse::<u32>().unwrap_or(0);
                let minor = version_parts[1].parse::<u32>().unwrap_or(0);
                let patch = version_parts[2].parse::<u32>().unwrap_or(0);

                return Version { major, minor, patch }
            }
        }
    }

    // if we can't get the latest release, return a default version
    Version { major: 0, minor: 0, patch: 0 }
}

impl Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let version_string = format!("{}.{}.{}", self.major, self.minor, self.patch);
        write!(f, "{}", version_string)
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

    /// equal to
    pub fn eq(&self, other: &Version) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch == other.patch
    }

    /// not equal to
    pub fn ne(&self, other: &Version) -> bool {
        self.major != other.major || self.minor != other.minor || self.patch != other.patch
    }
}

#[cfg(test)]
mod tests {
    use crate::utils::version::*;

    #[test]
    fn test_greater_than() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 3 };
        let v3 = Version { major: 2, minor: 2, patch: 5 };
        let v4 = Version { major: 1, minor: 4, patch: 4 };

        assert!(v1.gt(&v2));
        assert!(v1.gt(&v3));
        assert!(v1.gt(&v4));
        assert!(!v2.gt(&v1));
        assert!(!v1.gt(&v1));
    }

    #[test]
    fn test_greater_than_or_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 4 };

        assert!(v1.gte(&v2));
        assert!(v2.gte(&v1));
        assert!(v1.gte(&Version { major: 1, minor: 0, patch: 0 }));
    }

    #[test]
    fn test_less_than() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 5 };
        let v3 = Version { major: 2, minor: 4, patch: 4 };
        let v4 = Version { major: 3, minor: 3, patch: 4 };

        assert!(v1.lt(&v2));
        assert!(v1.lt(&v3));
        assert!(v1.lt(&v4));
        assert!(!v2.lt(&v1));
        assert!(!v1.lt(&v1));
    }

    #[test]
    fn test_less_than_or_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 4 };

        assert!(v1.lte(&v2));
        assert!(v2.lte(&v1));
        assert!(v1.lte(&Version { major: 3, minor: 0, patch: 0 }));
    }

    #[test]
    fn test_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 4 };
        let v3 = Version { major: 2, minor: 3, patch: 5 };

        assert!(v1.eq(&v2));
        assert!(!v1.eq(&v3));
    }

    #[test]
    fn test_not_equal_to() {
        let v1 = Version { major: 2, minor: 3, patch: 4 };
        let v2 = Version { major: 2, minor: 3, patch: 5 };
        let v3 = Version { major: 3, minor: 3, patch: 4 };

        assert!(v1.ne(&v2));
        assert!(v1.ne(&v3));
        assert!(!v1.ne(&Version { major: 2, minor: 3, patch: 4 }));
    }

    #[test]
    fn test_version_display() {
        let version = Version { major: 2, minor: 3, patch: 4 };

        assert_eq!(version.to_string(), "2.3.4");
    }

    #[test]
    fn test_version_current() {
        let version = current_version();

        assert_eq!(version.to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn test_version_remote() {}
}

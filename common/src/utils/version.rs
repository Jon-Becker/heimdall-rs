use std::fmt::Display;

use super::http::get_json_from_url;

#[derive(Debug)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

pub fn current_version() -> Version {
    // get the current version from the cargo package
    let version_string = env!("CARGO_PKG_VERSION");
    let version_parts: Vec<&str> = version_string.split('.').collect();

    Version {
        major: version_parts[0].parse::<u32>().unwrap_or(0),
        minor: version_parts[1].parse::<u32>().unwrap_or(0),
        patch: version_parts[2].parse::<u32>().unwrap_or(0),
    }
}

pub fn remote_version() -> Version {
    // get the latest release from github
    let remote_repository_url =
        "https://api.github.com/repos/Jon-Becker/heimdall-rs/releases/latest";

    // retrieve the latest release tag from github
    if let Some(release) = get_json_from_url(remote_repository_url.to_string()) {
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
    // greater than
    pub fn gt(&self, other: &Version) -> bool {
        self.major > other.major ||
            (self.major == other.major && self.minor > other.minor) ||
            (self.major == other.major && self.minor == other.minor && self.patch > other.patch)
    }

    // greater than or equal to
    pub fn gte(&self, other: &Version) -> bool {
        self.major > other.major ||
            (self.major == other.major && self.minor > other.minor) ||
            (self.major == other.major && self.minor == other.minor && self.patch >= other.patch)
    }

    // less than
    pub fn lt(&self, other: &Version) -> bool {
        self.major < other.major ||
            (self.major == other.major && self.minor < other.minor) ||
            (self.major == other.major && self.minor == other.minor && self.patch < other.patch)
    }

    // less than or equal to
    pub fn lte(&self, other: &Version) -> bool {
        self.major < other.major ||
            (self.major == other.major && self.minor < other.minor) ||
            (self.major == other.major && self.minor == other.minor && self.patch <= other.patch)
    }

    // equal to
    pub fn eq(&self, other: &Version) -> bool {
        self.major == other.major && self.minor == other.minor && self.patch == other.patch
    }

    // not equal to
    pub fn ne(&self, other: &Version) -> bool {
        self.major != other.major || self.minor != other.minor || self.patch != other.patch
    }
}

use std::fmt;

/// A Solidity data location attached to a reference type.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum DataLocation {
    Memory,
    Storage,
    Calldata,
}

/// Structural representation of a Solidity type used during postprocessing.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum SolidityType {
    Address,
    Bool,
    Uint(u16),
    Int(u16),
    FixedBytes(u8),
    Bytes,
    String,
    Mapping { key: Box<Self>, value: Box<Self> },
    Array { element: Box<Self>, length: Option<usize> },
    Located { ty: Box<Self>, location: DataLocation },
    Custom(String),
    Unknown,
}

impl SolidityType {
    /// Parse a type received at an ABI or legacy IR string boundary.
    pub(crate) fn parse(input: &str) -> Self {
        let input = input.trim();
        for (suffix, location) in [
            (" memory", DataLocation::Memory),
            (" storage", DataLocation::Storage),
            (" calldata", DataLocation::Calldata),
        ] {
            if let Some(ty) = input.strip_suffix(suffix) {
                return Self::Located { ty: Box::new(Self::parse(ty)), location }
            }
        }

        if let Some((key, value)) = mapping_parts(input) {
            return Self::Mapping {
                key: Box::new(Self::parse(key)),
                value: Box::new(Self::parse(value)),
            }
        }
        if let Some((element, length)) = array_parts(input) {
            return Self::Array { element: Box::new(Self::parse(element)), length }
        }

        match input {
            "address" => Self::Address,
            "bool" => Self::Bool,
            "bytes" => Self::Bytes,
            "string" => Self::String,
            "uint" => Self::Uint(256),
            "int" => Self::Int(256),
            _ if input.starts_with("uint") => input[4..]
                .parse::<u16>()
                .ok()
                .map(Self::Uint)
                .unwrap_or_else(|| Self::Custom(input.to_string())),
            _ if input.starts_with("int") => input[3..]
                .parse::<u16>()
                .ok()
                .map(Self::Int)
                .unwrap_or_else(|| Self::Custom(input.to_string())),
            _ if input.starts_with("bytes") => input[5..]
                .parse::<u8>()
                .ok()
                .map(Self::FixedBytes)
                .unwrap_or_else(|| Self::Custom(input.to_string())),
            "" => Self::Unknown,
            _ => Self::Custom(input.to_string()),
        }
    }

    pub(crate) fn in_memory(self) -> Self {
        Self::Located { ty: Box::new(self), location: DataLocation::Memory }
    }

    /// Return the element/value type produced by one indexing operation.
    pub(crate) fn indexed(&self) -> Self {
        match self.without_location() {
            Self::Mapping { value, .. } => *value,
            Self::Array { element, .. } => *element,
            _ => Self::Unknown,
        }
    }

    /// Remove data-location wrappers for semantic ABI/type comparisons.
    pub(crate) fn without_location(&self) -> Self {
        match self {
            Self::Located { ty, .. } => ty.without_location(),
            _ => self.clone(),
        }
    }

    pub(crate) fn is_mapping(&self) -> bool {
        matches!(self.without_location(), Self::Mapping { .. })
    }
}

fn mapping_parts(input: &str) -> Option<(&str, &str)> {
    let inner = input.strip_prefix("mapping(")?.strip_suffix(')')?;
    let mut depth = 0usize;
    for (index, byte) in inner.as_bytes().iter().enumerate() {
        match byte {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth = depth.checked_sub(1)?,
            b'=' if depth == 0 && inner.as_bytes().get(index + 1) == Some(&b'>') => {
                let key = inner[..index].trim();
                let value = inner[index + 2..].trim();
                return (!key.is_empty() && !value.is_empty()).then_some((key, value))
            }
            _ => {}
        }
    }
    None
}

fn array_parts(input: &str) -> Option<(&str, Option<usize>)> {
    let open = input.rfind('[')?;
    let length = input.strip_suffix(']')?.get(open + 1..)?;
    let length = if length.is_empty() { None } else { Some(length.parse().ok()?) };
    let element = input[..open].trim();
    (!element.is_empty()).then_some((element, length))
}

impl fmt::Display for SolidityType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Address => formatter.write_str("address"),
            Self::Bool => formatter.write_str("bool"),
            Self::Uint(width) => write!(formatter, "uint{width}"),
            Self::Int(width) => write!(formatter, "int{width}"),
            Self::FixedBytes(size) => write!(formatter, "bytes{size}"),
            Self::Bytes => formatter.write_str("bytes"),
            Self::String => formatter.write_str("string"),
            Self::Mapping { key, value } => write!(formatter, "mapping({key} => {value})"),
            Self::Array { element, length: Some(length) } => {
                write!(formatter, "{element}[{length}]")
            }
            Self::Array { element, length: None } => write!(formatter, "{element}[]"),
            Self::Located { ty, location } => {
                let location = match location {
                    DataLocation::Memory => "memory",
                    DataLocation::Storage => "storage",
                    DataLocation::Calldata => "calldata",
                };
                write!(formatter, "{ty} {location}")
            }
            Self::Custom(name) => formatter.write_str(name),
            Self::Unknown => formatter.write_str("bytes32"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_indexes_nested_mapping() {
        let ty = SolidityType::parse("mapping(address => mapping(uint256 => bool))");
        assert!(matches!(ty, SolidityType::Mapping { .. }));
        assert!(matches!(ty.indexed(), SolidityType::Mapping { .. }));
        assert_eq!(ty.indexed().indexed(), SolidityType::Bool);
        assert_eq!(ty.to_string(), "mapping(address => mapping(uint256 => bool))");
    }

    #[test]
    fn preserves_array_length_and_data_location() {
        let ty = SolidityType::parse("bytes32[4][] memory");
        assert_eq!(ty.to_string(), "bytes32[4][] memory");
        assert_eq!(ty.without_location().indexed().indexed(), SolidityType::FixedBytes(32));
    }
}

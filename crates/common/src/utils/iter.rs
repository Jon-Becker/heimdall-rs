/// Extension trait for byte slices that adds helpful operations.
pub trait ByteSliceExt {
    /// Splits a byte slice by a delimiter byte slice.
    ///
    /// # Arguments
    ///
    /// * `delimiter` - The byte sequence to split on
    ///
    /// # Returns
    ///
    /// * `Vec<&[u8]>` - The split parts
    fn split_by_slice(&self, delimiter: &[u8]) -> Vec<&[u8]>;

    /// Checks if a byte slice contains another byte slice.
    ///
    /// # Arguments
    ///
    /// * `sequence` - The byte sequence to search for
    ///
    /// # Returns
    ///
    /// * `bool` - `true` if the sequence is found, `false` otherwise
    fn contains_slice(&self, sequence: &[u8]) -> bool;
}

impl ByteSliceExt for [u8] {
    fn split_by_slice(&self, delimiter: &[u8]) -> Vec<&[u8]> {
        if self.is_empty() {
            return vec![];
        }

        // if the delimiter is empty, return each byte as a separate slice
        if delimiter.is_empty() {
            let mut parts = Vec::with_capacity(self.len());
            for i in 0..self.len() {
                parts.push(&self[i..i + 1]);
            }
            return parts;
        }

        let mut parts = Vec::new();
        let mut start = 0;
        for (i, _) in self.windows(delimiter.len()).enumerate() {
            if self[i..i + delimiter.len()] == *delimiter {
                parts.push(&self[start..i]);
                start = i + delimiter.len();
            }
        }

        parts.push(&self[start..]);
        parts
    }

    fn contains_slice(&self, sequence: &[u8]) -> bool {
        self.windows(sequence.len()).any(|window| window == sequence)
    }
}

/// Removes elements at specified indices from a collection.
///
/// This function takes a collection and a sorted list of indices, and removes
/// the elements at those indices from the collection.
///
/// # Arguments
///
/// * `v` - The collection to remove elements from
/// * `indices` - A sorted list of indices to remove
///
/// # Returns
///
/// * `Vec<T>` - A new collection with the elements at the specified indices removed
pub fn remove_sorted_indices<T>(
    v: impl IntoIterator<Item = T>,
    indices: impl IntoIterator<Item = usize>,
) -> Vec<T> {
    let v = v.into_iter();
    let mut indices = indices.into_iter();
    let mut i = match indices.next() {
        None => return v.collect(),
        Some(i) => i,
    };
    let (min, max) = v.size_hint();
    let mut result = Vec::with_capacity(max.unwrap_or(min));

    for (j, x) in v.into_iter().enumerate() {
        if j == i {
            if let Some(idx) = indices.next() {
                i = idx;
            }
        } else {
            result.push(x);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_sorted_indices() {
        assert_eq!(remove_sorted_indices(vec![1, 2, 3, 4, 5], vec![0, 2, 4]), vec![2, 4]);
    }

    #[test]
    fn test_remove_sorted_indices_empty() {
        assert_eq!(remove_sorted_indices(vec![1, 2, 3, 4, 5], vec![]), vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_contains_empty_slice() {
        let data: &[u8] = &[];
        let sequence = &[1, 2];
        assert!(!data.contains_slice(sequence));
    }

    #[test]
    fn test_contains_no_match() {
        let data = &[1, 2, 3];
        let sequence = &[4, 5];
        assert!(!data.contains_slice(sequence));
    }

    #[test]
    fn test_contains_single_match() {
        let data = &[1, 2, 3, 4, 5];
        let sequence = &[3, 4];
        assert!(data.contains_slice(sequence));
    }

    #[test]
    fn test_contains_multiple_matches() {
        let data = &[1, 2, 3, 2, 3, 4];
        let sequence = &[2, 3];
        assert!(data.contains_slice(sequence));
    }

    #[test]
    fn test_contains_sequence_at_start() {
        let data = &[1, 2, 3, 4, 5];
        let sequence = &[1, 2];
        assert!(data.contains_slice(sequence));
    }

    #[test]
    fn test_contains_sequence_at_end() {
        let data = &[1, 2, 3, 4, 5];
        let sequence = &[4, 5];
        assert!(data.contains_slice(sequence));
    }

    #[test]
    fn test_contains_with_vec() {
        let data = [1, 2, 3, 4, 5];
        let sequence = &[3, 4];
        assert!(data.contains_slice(sequence));
    }

    #[test]
    fn test_contains_sequence_identical_to_data() {
        let data = &[1, 2, 3];
        let sequence = &[1, 2, 3];
        assert!(data.contains_slice(sequence));
    }

    #[test]
    fn test_split_by_slice_empty_data() {
        let data: &[u8] = &[];
        let delimiter = &[1, 2];
        assert!(data.split_by_slice(delimiter).is_empty());
    }

    #[test]
    fn test_split_by_slice_empty_delimiter() {
        let data = &[1, 2, 3];
        let delimiter: &[u8] = &[];
        assert_eq!(data.split_by_slice(delimiter), vec![&[1], &[2], &[3]]);
    }

    #[test]
    fn test_split_by_slice_no_match() {
        let data = &[1, 2, 3];
        let delimiter = &[4, 5];
        assert_eq!(data.split_by_slice(delimiter), vec![&[1, 2, 3]]);
    }

    #[test]
    fn test_split_by_slice_single_match() {
        let data = &[1, 2, 3, 4, 5];
        let delimiter = &[3, 4];

        let expected: Vec<&[u8]> = vec![&[1, 2], &[5]];

        assert_eq!(data.split_by_slice(delimiter), expected);
    }

    #[test]
    fn test_split_by_slice_multiple_matches() {
        let data = &[1, 2, 3, 2, 3, 4];
        let delimiter = &[2, 3];

        let expected: Vec<&[u8]> = vec![&[1], &[], &[4]];

        assert_eq!(data.split_by_slice(delimiter), expected);
    }

    #[test]
    fn test_split_by_slice_sequence_at_start() {
        let data = &[1, 2, 3, 4, 5];
        let delimiter = &[1, 2];

        let expected: Vec<&[u8]> = vec![&[], &[3, 4, 5]];

        assert_eq!(data.split_by_slice(delimiter), expected);
    }
}

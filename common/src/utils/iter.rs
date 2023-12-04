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
}

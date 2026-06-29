use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Version {
    value: String,
}

impl From<String> for Version {
    fn from(value: String) -> Self {
        Version { value }
    }
}

impl From<Version> for String {
    fn from(version: Version) -> Self {
        version.value
    }
}

impl Version {
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut a = self.as_str();
        let mut b = other.as_str();

        loop {
            if a.is_empty() && b.is_empty() {
                return Ordering::Equal;
            }
            if a.is_empty() {
                return Ordering::Less;
            }
            if b.is_empty() {
                return Ordering::Greater;
            }

            match (unsplice_number(a), unsplice_number(b)) {
                (Some((a_num, a_rem)), Some((b_num, b_rem))) => {
                    if a_num != b_num {
                        return a_num.cmp(&b_num);
                    }
                    a = a_rem;
                    b = b_rem;
                }
                (Some(_), None) => return Ordering::Greater,
                (None, Some(_)) => return Ordering::Less,
                (None, None) => {
                    let (a_chunk, a_rem) = unsplice_cond(a, |c| !c.is_ascii_digit());
                    let (b_chunk, b_rem) = unsplice_cond(b, |c| !c.is_ascii_digit());
                    if a_chunk != b_chunk {
                        return a_chunk.cmp(b_chunk);
                    }
                    a = a_rem;
                    b = b_rem;
                }
            }
        }
    }
}

fn unsplice_cond<F: Fn(char) -> bool>(v: &str, f: F) -> (&str, &str) {
    let mut matched = "";
    let mut rem = v;

    for (idx, c) in v.char_indices() {
        if f(c) {
            let idx = idx.saturating_add(c.len_utf8());
            let (a, b) = v.split_at(idx);
            matched = a;
            rem = b;
        } else {
            break;
        }
    }

    (matched, rem)
}

fn unsplice_number(v: &str) -> Option<(u64, &str)> {
    let (num, rem) = unsplice_cond(v, |c| c.is_ascii_digit());
    let num = num.parse::<u64>().ok()?;
    Some((num, rem))
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_serde() {
        let version = Version::from("1.2.3".to_string());
        let serialized = serde_json::to_string(&version).unwrap();
        assert_eq!(serialized, r#""1.2.3""#);

        let deserialized = serde_json::from_str::<Version>(&serialized).unwrap();
        assert_eq!(deserialized, version);
    }

    #[test]
    fn test_alpine() {
        let a = Version::from("6.18.35-0-virt".to_string());
        let b = Version::from("6.18.36-0-virt".to_string());
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(b.cmp(&a), Ordering::Greater);
        assert_eq!(a.cmp(&a), Ordering::Equal);
        assert_eq!(b.cmp(&b), Ordering::Equal);
    }

    #[test]
    fn test_numeric_sort() {
        let a = Version::from("1.2.3".to_string());
        let b = Version::from("1.2.10".to_string());
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(b.cmp(&a), Ordering::Greater);
    }

    #[test]
    fn test_unsplice_number() {
        assert_eq!(unsplice_number("123abc"), Some((123, "abc")));
        assert_eq!(unsplice_number("456"), Some((456, "")));
        assert_eq!(unsplice_number("78.9"), Some((78, ".9")));
        assert_eq!(unsplice_number("abc"), None);
        assert_eq!(unsplice_number(""), None);
    }

    #[test]
    fn test_unsplice_cond() {
        assert_eq!(unsplice_cond("AABBCC", |c| c == 'A'), ("AA", "BBCC"));
        assert_eq!(
            unsplice_cond("123abc", |c| c.is_ascii_digit()),
            ("123", "abc")
        );
        assert_eq!(unsplice_cond("a1b2c3", |_c| true), ("a1b2c3", ""));
        assert_eq!(unsplice_cond("a1b2c3", |_c| false), ("", "a1b2c3"));
    }

    #[test]
    fn test_arch_suffix() {
        let a = Version::from("7.0.14-arch1-1".to_string());
        let b = Version::from("7.0.14-arch1-2".to_string());
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(b.cmp(&a), Ordering::Greater);

        // Not sure this ever happens, but make sure it would work
        let a = Version::from("7.0.14-arch1-1".to_string());
        let b = Version::from("7.0.14-arch2-1".to_string());
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(b.cmp(&a), Ordering::Greater);
    }

    #[test]
    fn test_hardened_kernel() {
        let a = Version::from("6.17.3-hardened1-3-hardened".to_string());
        let b = Version::from("7.0.12-hardened1-2-hardened".to_string());
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(b.cmp(&a), Ordering::Greater);

        // Test identical upstream versions
        let a = Version::from("7.0.12-hardened1-1-hardened".to_string());
        let b = Version::from("7.0.12-hardened1-2-hardened".to_string());
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(b.cmp(&a), Ordering::Greater);
    }

    #[test]
    #[ignore = "We don't have a solution for this yet, unrelated folders should not win version compare"]
    fn test_old_kernel() {
        let a = Version::from("Linux .old".to_string());
        let b = Version::from("Linux 7.0.12-arch1-1".to_string());
        assert_eq!(a.cmp(&b), Ordering::Less);
        assert_eq!(b.cmp(&a), Ordering::Greater);
    }
}

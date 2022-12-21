use opener::open;

// If we try to open a non-existant path, opener should return some
// kind of error
#[test]
fn test_missing_file() {
    assert!(open("non_existant_path").is_err());
}

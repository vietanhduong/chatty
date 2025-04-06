use super::*;

#[test]
fn test_model_filter_build() {
    let filter = ModelFilter::Equals("gpt-4".to_string());
    let regex = filter.build().expect("build regex");
    assert!(regex.is_match("gpt-4"));
    assert!(!regex.is_match("gpt-4-turbo"));

    let filter = ModelFilter::Contains("gpt-4".to_string());
    let regex = filter.build().expect("build regex");
    assert!(regex.is_match("gpt-4"));
    assert!(regex.is_match("gpt-4-turbo"));
    assert!(!regex.is_match("gpt-3"));

    let filter = ModelFilter::Regex("^(gpt-4|gpt-3)$".to_string());
    let regex = filter.build().expect("build regex");
    assert!(regex.is_match("gpt-4"));
    assert!(regex.is_match("gpt-3"));
    assert!(!regex.is_match("gpt-4-turbo"));
    assert!(!regex.is_match("gpt-3-turbo"));
}

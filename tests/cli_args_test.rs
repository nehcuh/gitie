use gitie::cli_interface::args::{args_contain_help, args_contain_ai, args_contain_noai, should_use_ai};

#[test]
fn test_args_contain_help() {
    // Test with -h
    let args = vec!["-h".to_string()];
    assert!(args_contain_help(&args));

    // Test with --help
    let args = vec!["--help".to_string()];
    assert!(args_contain_help(&args));

    // Test with -h in middle
    let args = vec!["commit".to_string(), "-h".to_string(), "--all".to_string()];
    assert!(args_contain_help(&args));

    // Test without help flags
    let args = vec!["commit".to_string(), "--all".to_string()];
    assert!(!args_contain_help(&args));

    // Test with similar but not matching strings
    let args = vec!["--helps".to_string(), "-help".to_string()];
    assert!(!args_contain_help(&args));
}

#[test]
fn test_args_contain_ai() {
    // Test with --ai
    let args = vec!["--ai".to_string()];
    assert!(args_contain_ai(&args));

    // Test with --ai in middle
    let args = vec!["commit".to_string(), "--ai".to_string(), "--all".to_string()];
    assert!(args_contain_ai(&args));

    // Test without ai flag
    let args = vec!["commit".to_string(), "--all".to_string()];
    assert!(!args_contain_ai(&args));

    // Test with similar but not matching strings
    let args = vec!["--artificial-intelligence".to_string(), "-ai".to_string()];
    assert!(!args_contain_ai(&args));
}

#[test]
fn test_args_contain_noai() {
    // Test with --noai
    let args = vec!["--noai".to_string()];
    assert!(args_contain_noai(&args));

    // Test with --noai in middle
    let args = vec!["commit".to_string(), "--noai".to_string(), "--all".to_string()];
    assert!(args_contain_noai(&args));

    // Test without noai flag
    let args = vec!["commit".to_string(), "--all".to_string()];
    assert!(!args_contain_noai(&args));

    // Test with similar but not matching strings
    let args = vec!["--no-ai".to_string(), "-noai".to_string()];
    assert!(!args_contain_noai(&args));
}

#[test]
fn test_should_use_ai() {
    // Default behavior: AI enabled
    let args = vec!["commit".to_string()];
    assert!(should_use_ai(&args));

    // With --ai flag (backward compatibility)
    let args = vec!["commit".to_string(), "--ai".to_string()];
    assert!(should_use_ai(&args));

    // With --noai flag: AI disabled
    let args = vec!["commit".to_string(), "--noai".to_string()];
    assert!(!should_use_ai(&args));

    // With both --ai and --noai: --noai should take precedence
    let args = vec!["commit".to_string(), "--ai".to_string(), "--noai".to_string()];
    assert!(!should_use_ai(&args));

    // With both --noai and --ai in different order: --noai should still take precedence
    let args = vec!["commit".to_string(), "--noai".to_string(), "--ai".to_string()];
    assert!(!should_use_ai(&args));

    // With multiple instances of --ai: AI should still be enabled
    let args = vec!["commit".to_string(), "--ai".to_string(), "--ai".to_string()];
    assert!(should_use_ai(&args));

    // With multiple instances of --noai: AI should be disabled
    let args = vec!["commit".to_string(), "--noai".to_string(), "--noai".to_string()];
    assert!(!should_use_ai(&args));
}
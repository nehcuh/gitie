// nops.rs - No-operation utilities for Gitie
//
// This module provides placeholder functions and utilities that perform no actual operations.
// They can be useful for testing, stubbing, or as default implementations.

/// Performs no operation.
///
/// # Returns
///
/// * `()` - Unit type, indicating no return value
pub fn nop() {}

/// Performs no operation but returns a Result with Ok.
///
/// # Type Parameters
///
/// * `T` - The success type to wrap in Ok
/// * `E` - The error type (unused)
///
/// # Returns
///
/// * `Result<T, E>` - Always returns Ok with the provided value
pub fn nop_ok<T, E>(value: T) -> Result<T, E> {
    Ok(value)
}

/// Performs no operation but returns a Result with Err.
///
/// # Type Parameters
///
/// * `T` - The success type (unused)
/// * `E` - The error type to wrap in Err
///
/// # Returns
///
/// * `Result<T, E>` - Always returns Err with the provided error
pub fn nop_err<T, E>(error: E) -> Result<T, E> {
    Err(error)
}

/// Performs no operation but returns Some.
///
/// # Type Parameters
///
/// * `T` - The type to wrap in Some
///
/// # Returns
///
/// * `Option<T>` - Always returns Some with the provided value
pub fn nop_some<T>(value: T) -> Option<T> {
    Some(value)
}

/// Performs no operation and returns None.
///
/// # Type Parameters
///
/// * `T` - The type parameter for Option (unused)
///
/// # Returns
///
/// * `Option<T>` - Always returns None
pub fn nop_none<T>() -> Option<T> {
    None
}

/// A struct that does nothing.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Nop;

impl Nop {
    /// Creates a new Nop instance.
    pub fn new() -> Self {
        Nop
    }
    
    /// Does nothing.
    pub fn do_nothing(&self) {}
    
    /// Returns the provided value unchanged.
    pub fn identity<T>(value: T) -> T {
        value
    }
}
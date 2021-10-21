pub mod gen;

#[cfg(not(any(test, target_family = "wasm")))]
pub mod py;

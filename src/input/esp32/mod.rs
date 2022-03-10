mod gpio;

#[cfg(feature = "lca2021_badge")]
mod touch;

#[cfg(feature = "lca2021_badge")]
pub use touch::TouchControllerBuilder;

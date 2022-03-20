use std::fmt::Debug;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ButtonId {
    Physical(usize),
    Controller(usize),
    PageUp,
    PageDown,
}

#[cfg(feature = "lca2021_badge")]
pub mod touch;

#[cfg(any(feature = "lca2021_badge", feature = "robotica"))]
pub mod gpio;

use std::fmt::Debug;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(dead_code)]
pub enum ButtonId {
    Physical(usize),
    Controller(usize),
    PageUp,
    PageDown,
    NotAButton,
}

#[cfg(feature = "lca2021_badge")]
pub mod touch;

#[cfg(any(feature = "lca2021_badge", feature = "robotica"))]
pub mod gpio;

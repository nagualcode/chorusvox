//! DSP do Heave, independente do framework de plugin (usado pelo plugin, pelo CLI e pelos testes).

pub mod engine;
pub mod harmony;
pub mod marks;
pub mod pitch;
pub mod psola;
pub mod util;

pub use engine::{Engine, EngineParams};

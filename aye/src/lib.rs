mod assignment;
mod batch;
mod bypass;
mod cli;
mod domain;
pub mod error;
pub mod model;
mod projection;
pub mod reader;
mod store;
mod sync;

pub use bypass::BYPASSED_LABEL;
pub use cli::run_cli;

pub fn run() {
    if !bypass::run_if_requested() {
        run_cli();
    }
}

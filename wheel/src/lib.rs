#![allow(clippy::borrow_deref_ref)] // https://github.com/rust-lang/rust-clippy/issues/8971

mod adapt_response;
mod api;
mod coin;
mod coin_state;
mod from_json_dict;
mod respond_to_ph_updates;
mod run_generator;
mod run_program;
mod to_json_dict;

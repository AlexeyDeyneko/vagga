#![feature(path_ext)]

extern crate shaman;
extern crate libc;
extern crate nix;
extern crate rand;
extern crate rustc_serialize;
#[macro_use] extern crate log;
extern crate env_logger;
extern crate argparse;
extern crate quire;
extern crate time;

#[macro_use] mod macros;
mod builder;
mod config;
mod container;
mod launcher;
mod network;
mod setup_netns;
mod version;
mod wrapper;
mod file_util;
mod path_util;

fn main() {
    env_logger::init().unwrap();
}

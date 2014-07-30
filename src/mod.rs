#![feature(phase)]
#![feature(macro_rules)]  // for sha256 from librustc

extern crate serialize;
extern crate collections;
extern crate debug;
extern crate libc;
extern crate green;
extern crate rustuv;
extern crate regex;
#[phase(plugin, link)] extern crate log;
#[phase(plugin)] extern crate regex_macros;

extern crate quire;
extern crate argparse;


use std::os::set_exit_status;

use self::main::run;
use self::linux::init_prctl;


mod config;
mod build;
mod run;
mod chroot;
mod env;
mod main;
mod linux;
mod options;
mod settings;
mod yamlutil;
mod monitor;
mod sha256;

mod commands {
    pub mod shell;
    pub mod command;
    pub mod supervise;
}


fn main() {
    init_prctl();
    set_exit_status(run());
}

#[macro_use]
extern crate clap;

use std::{
    thread,
    time::Duration
};

use clap::{Arg, App};
use cpu::*;

mod cpu;

static FORMAT: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

fn main() {
    let matches = App::new("cpuline")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(Arg::with_name("interval")
             .global(true)
             .short("i")
             .long("interval")
             .value_name("MS")
             .takes_value(true)
             .default_value("1000"))
        .get_matches();

    let interval = value_t!(matches, "interval", u64).unwrap();

    let mut stat = None;

    loop {
        let old = stat;
        stat = Stat::read().ok();

        match (&stat, &old) {
            (&Some(ref now), &Some(ref old)) => {
                let load = now.load_since(&old);

                for (_, core) in load.cores.iter() {
                    // How much this core was used with 0 (not used) to 1 (fully used)
                    let used_part = core.busy_time() as f32 / core.total_time() as f32;
                    let used_part = used_part.max(0.).min(1.);

                    let output = FORMAT[((FORMAT.len() - 1) as f32 * used_part) as usize];
                    print!("{}", output);
                }
                println!("");
            },
            _ => ()
        }

        thread::sleep(Duration::from_millis(interval));
    }
}

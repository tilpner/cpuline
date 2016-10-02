#[macro_use]
extern crate clap;
extern crate num;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{self, Read};
use std::{iter, thread, ops};
use std::time::Duration;

use clap::{Arg, App, AppSettings, SubCommand};

use num::NumCast;

const SYS_PRESENT: &'static str = "/sys/devices/system/cpu/present";
static FORMAT: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

const WINDOW_SIZE: usize = 32;

#[derive(Debug, Default)]
struct Window<T> {
    data: Vec<T>,
    idx: usize,
    size: usize
}

impl<T> Window<T> where T: for<'a> iter::Sum<&'a T> + ops::Div<Output=T> + NumCast {
    fn new(size: usize) -> Window<T> {
        Window { data: Vec::with_capacity(size), idx: 0, size: size }
    }

    fn sample(&mut self, value: T) {
        if self.data.len() < self.size { self.data.push(value) }
        else { self.data[self.idx] = value; }
        self.idx = (self.idx + 1) % self.size;
    }

    fn elements(&self) -> (&[T], &[T]) { (&self.data[self.idx..], &self.data[..self.idx]) }
    fn average(&self) -> T { self.data.iter().sum::<T>() / T::from(self.data.len()).unwrap() }
}

#[derive(Debug, Default)]
struct Core {
    index: u32,
    min_freq: u32,
    max_freq: u32,
    cur_freq: Window<u32>
}

#[derive(Debug)]
struct System {
    cores: Vec<Core>,
    load: Window<f32>
}

impl System {
    fn new() -> System {
        System {
            cores: init_cores(),
            load: Window::new(8)
        }
    }

    // Direct printing to avoid allocation
    fn print_cores(&self) {
        for c in &self.cores {
            let load = (c.cur_freq.average() - c.min_freq) as f32 / (c.max_freq - c.min_freq) as f32;
            print!("{}", FORMAT[(FORMAT.len() as f32 * load) as usize]);
        }
        println!("");
    }

    fn print_system(&self) {
        let (first, last) = self.load.elements();
        for load in first.iter().chain(last.iter()) {
            print!("{}", FORMAT[(FORMAT.len() as f32 * load) as usize]);
        }
        println!("");
    }
}

fn read_into_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut output = String::new();
    try!(try!(File::open(path)).read_to_string(&mut output));
    Ok(output)
}

fn init_cores() -> Vec<Core> {
    let present = read_into_string(SYS_PRESENT).expect("Can't read kernel interface to query present cores");
    
    // a-b
    let mut parts = present.split('-').flat_map(|s| s.trim().parse::<u32>().ok());
    let (a, b) = (parts.next().unwrap(), parts.next().unwrap());
    (a..b + 1).map(|idx| {
        let cpu_path: PathBuf = format!("/sys/devices/system/cpu/cpu{}/cpufreq", idx).into();
        Core {
            index: idx,
            min_freq: read_into_string(cpu_path.join("scaling_min_freq")).ok()
                .and_then(|s| s.trim().parse().ok()).expect("Can't read min freq"),
            max_freq: read_into_string(cpu_path.join("scaling_max_freq")).ok()
                .and_then(|s| s.trim().parse().ok()).expect("Can't read max frequency"),
            cur_freq: Window::new(WINDOW_SIZE)
        }
    }).collect()
}

fn update(system: &mut System) {
    let mut frame_average = 0.;
    for c in &mut system.cores {
        let cpu_path: PathBuf = format!("/sys/devices/system/cpu/cpu{}/cpufreq/scaling_cur_freq", c.index).into();
        let cur_freq = read_into_string(cpu_path).ok()
                .and_then(|s| s.trim().parse().ok()).expect("Can't read current frequency");
        c.cur_freq.sample(cur_freq);
        frame_average += (cur_freq - c.min_freq) as f32 / (c.max_freq - c.min_freq) as f32;
    }
    system.load.sample(frame_average / system.cores.len() as f32);
}

fn main() {
    let matches = App::new("cpuline")
        .version(crate_version!())
        .author(crate_authors!())
        .about("Display CPU usage per-core or over time (Linux-only)")
        .setting(AppSettings::SubcommandRequired)
        .arg(Arg::with_name("interval")
             .global(true)
             .short("i")
             .long("interval")
             .value_name("MS")
             .takes_value(true)
             .default_value("1000"))
        .subcommand(SubCommand::with_name("cores"))
        .subcommand(SubCommand::with_name("time"))
        .get_matches();

    let action = match matches.subcommand_name() {
        Some("cores") => System::print_cores,
        Some("time") => System::print_system,
        _ => unreachable!()
    };

    let interval = value_t!(matches, "interval", u64).unwrap();

    let mut system = System::new();

    loop {
        update(&mut system);
        action(&system);
        thread::sleep(Duration::from_millis(interval));
    }
}

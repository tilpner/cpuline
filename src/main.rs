#[macro_use]
extern crate clap;
extern crate vec_map;
extern crate libc;

#[macro_use]
extern crate log;
extern crate env_logger;

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::thread;
use std::time::{Instant, Duration};

use clap::{Arg, App};

use vec_map::VecMap;

const PROC_STAT: &'static str = "/proc/stat";
static FORMAT: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// cpu  3357 0 4313 1362393
///   The  amount  of  time, measured in units of USER_HZ (1/100ths of a
///   second on most architectures, use sysconf(_SC_CLK_TCK)  to  obtain
///   the right value), that the system spent in various states:
///
///   user   (1) Time spent in user mode.
///
///   nice   (2) Time spent in user mode with low priority (nice).
///
///   system (3) Time spent in system mode.
///
///   idle   (4) Time  spent  in  the  idle task.  This value should be
///              USER_HZ times the second entry in the /proc/uptime  pseudo-
///              file.
///
///   iowait (since Linux 2.5.41)
///          (5) Time waiting for I/O to complete.
///
///   irq (since Linux 2.6.0-test4)
///          (6) Time servicing interrupts.
///
///   softirq (since Linux 2.6.0-test4)
///          (7) Time servicing softirqs.
///
///   steal (since Linux 2.6.11)
///          (8) Stolen time, which is the time spent in other operating
///              systems when running in a virtualized environment
///
///   guest (since Linux 2.6.24)
///          (9) Time spent running a virtual CPU  for  guest  operating
///              systems under the control of the Linux kernel.
///
///   guest_nice (since Linux 2.6.33)
///        (10)  Time  spent  running  a  niced guest (virtual CPU for
///              guest operating systems under the control of the Linux ker‐
///              nel).
#[derive(Debug)]
struct Stat {
    time: Instant,
    total: Option<CPU>,
    cores: VecMap<CPU>
}

impl Stat {
    pub fn read() -> io::Result<Stat> {
        let file = try!(File::open(PROC_STAT));
        let reader = BufReader::new(file);
        let mut stat = Stat { time: Instant::now(), total: None, cores: VecMap::new() };

        for line in reader.lines() {
            let line = try!(line);
            const OFFSET: usize = 3; // "cpu".len()
            if line.starts_with("cpu ") {
                stat.total = Some(CPU::from_line(&line[OFFSET..])); 
            } else if line.starts_with("cpu") {
                let num: u64 = line[OFFSET..].split_whitespace().next().and_then(|s| s.trim().parse().ok()).unwrap();
                stat.cores.insert(num as usize, CPU::from_line(&line[OFFSET..]));
            }
        }

        Ok(stat)
    }

    pub fn load_since(&self, earlier: &Stat) -> Load {
        Load {
            duration: self.time.duration_since(earlier.time),
            total: match (&self.total, &earlier.total) {
                (&Some(ref now), &Some(ref old)) => Some(now.diff(old)),
                _ => None
            },
            cores: self.cores.iter()
                .flat_map(|(idx, core)| earlier.cores.get(idx).map(|ec| (idx, core.diff(ec))))
                .collect()
        }
    }
}

#[derive(Debug, Clone)]
struct Load {
    duration: Duration,
    total: Option<CPU>,
    cores: VecMap<CPU>
}

#[derive(Debug, Clone)]
struct CPU {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64
}

impl CPU {
    pub fn from_line(line: &str) -> CPU {
        fn parse(s: Option<&str>) -> u64 { s.and_then(|s| s.trim().parse().ok()).expect("Couldn't parse CPU stat") }
        let mut tok = line.split_whitespace();
        CPU {
            user: parse(tok.next()),
            nice: parse(tok.next()),
            system: parse(tok.next()),
            idle: parse(tok.next())
        }
    }

    pub fn diff(&self, other: &CPU) -> CPU {
        CPU {
            user: self.user - other.user,
            nice: self.nice - other.nice,
            system: self.system - other.system,
            idle: self.idle - other.idle
        }
    }
}

fn main() {
    env_logger::init().unwrap();
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

    let user_hz: u64 = unsafe { libc::sysconf(libc::_SC_CLK_TCK) as u64 };
    debug!("user_hz({})", user_hz);
    
    loop {
        let old = stat;
        stat = Stat::read().ok();

        match (&stat, &old) {
            (&Some(ref now), &Some(ref old)) => {
                let load = now.load_since(&old);
                let duration_ticks = load.duration.as_secs() * user_hz 
                    + (load.duration.subsec_nanos() as f64 / 1E9 * user_hz as f64) as u64;

                for (_, core) in load.cores.iter() {
                    // How many ticks the core was in use
                    let used = core.user + core.nice + core.system;
                    // How long this core was used with 0 (not used) to 1 (fully used)
                    let used_part = used as f32 / duration_ticks as f32;

                    let output = FORMAT[((FORMAT.len() - 1) as f32 * used_part) as usize];
                    debug!("used_part({}) = used({}) / duration_ticks({}) => '{}'", used_part, used, duration_ticks, output);
                    print!("{}", output);
                }
                println!("");
            },
            _ => ()
        }

        thread::sleep(Duration::from_millis(interval));
    }
}

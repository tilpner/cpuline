extern crate vec_map;

use std::{
    fs::File,
    io::{ self, BufRead, BufReader }
};
use vec_map::VecMap;

const PROC_STAT: &'static str = "/proc/stat";

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
pub struct Stat {
    total: Option<CPU>,
    cores: VecMap<CPU>
}

impl Stat {
    pub fn read() -> io::Result<Stat> {
        let file = File::open(PROC_STAT)?;
        let reader = BufReader::new(file);
        let mut stat = Stat { total: None, cores: VecMap::new() };

        for line in reader.lines() {
            let line = line?;
            const OFFSET: usize = 3; // "cpu".len()
            if line.starts_with("cpu ") {
                stat.total = Some(CPU::from_line(&line[OFFSET..])); 
            } else if line.starts_with("cpu") {
                let first_space = line.find(' ').unwrap();
                let num: u64 = line[OFFSET..first_space].parse().unwrap();
                let cpu = CPU::from_line(&line[first_space..]);
                stat.cores.insert(num as usize, cpu);
            }
        }

        Ok(stat)
    }

    pub fn load_since(&self, earlier: &Stat) -> Load {
        Load {
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

#[derive(Debug, Default, Clone)]
pub struct Load {
    pub total: Option<CPU>,
    pub cores: VecMap<CPU>
}

#[derive(Debug, Clone)]
pub struct CPU {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
    guest: u64,
    guest_nice: u64
}

impl CPU {
    pub fn from_line(line: &str) -> CPU {
        fn parse(s: Option<&str>) -> u64 { s.and_then(|s| s.trim().parse().ok()).expect("Couldn't parse CPU stat") }
        let mut tok = line.split_whitespace();
        let user = parse(tok.next());
        let nice = parse(tok.next());
        let system = parse(tok.next());
        let idle = parse(tok.next());
        let iowait = parse(tok.next());
        let irq = parse(tok.next());
        let softirq = parse(tok.next());
        let steal = parse(tok.next());
        let guest = parse(tok.next());
        let guest_nice = parse(tok.next());

        CPU { user, nice, system, idle, iowait, irq, softirq, steal, guest, guest_nice }
    }

    pub fn diff(&self, other: &CPU) -> CPU {
        CPU {
            user: self.user - other.user,
            nice: self.nice - other.nice,
            system: self.system - other.system,
            idle: self.idle - other.idle,
            iowait: self.iowait - other.iowait,
            irq: self.irq - other.irq,
            softirq: self.softirq - other.softirq,
            steal: self.steal - other.steal,
            guest: self.guest - other.guest,
            guest_nice: self.guest_nice - other.guest_nice
        }
    }

    pub fn idle_time(&self) -> u64 { self.idle + self.iowait }
    // guest and guest_nice are already accounted for in user and nice
    pub fn user_time(&self) -> u64 { self.user + self.nice }
    pub fn system_time(&self) -> u64 { self.system + self.irq + self.softirq }
    pub fn busy_time(&self) -> u64 { self.user_time() + self.system_time() + self.steal }
    pub fn total_time(&self) -> u64 { self.busy_time() + self.idle_time() }
}

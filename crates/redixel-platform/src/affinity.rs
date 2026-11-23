//! Keeping the frame thread off the CPUs that service GPU interrupts.
//!
//! A GPU raises thousands of interrupts a second while frames are in flight
//! (fences, presents), and the kernel services each one on a fixed CPU. A
//! frame thread the scheduler places on that CPU is interrupted constantly, and
//! since the scheduler tends to leave a busy thread where it is, the whole run
//! can do its CPU-bound frame work at half speed. Moving the thread elsewhere
//! once, at startup, removes that.

/// Restricts the calling thread to the CPUs that do not service GPU
/// interrupts, and returns the CPUs it now runs on.
///
/// Returns `None`, leaving the affinity untouched, when it cannot tell which
/// CPUs those are, when avoiding them would leave none, or on any platform but
/// Linux.
pub fn keep_current_thread_off_gpu_irq_cpus() -> Option<Vec<usize>> {
    #[cfg(target_os = "linux")]
    {
        linux::keep_current_thread_off_gpu_irq_cpus()
    }

    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

#[cfg(target_os = "linux")]
mod linux {
    /// Interrupt names, as `/proc/interrupts` lists them, of the GPU drivers
    /// whose interrupts are worth avoiding.
    const GPU_DRIVERS: [&str; 4] = ["nvidia", "amdgpu", "i915", "xe"];

    pub(super) fn keep_current_thread_off_gpu_irq_cpus() -> Option<Vec<usize>> {
        let table: String = std::fs::read_to_string("/proc/interrupts").ok()?;
        let avoided: Vec<usize> = gpu_irq_cpus(&table, |irq: &str| {
            std::fs::read_to_string(format!("/proc/irq/{irq}/effective_affinity_list")).ok()
        });

        let total: usize = std::thread::available_parallelism().ok()?.get();
        let allowed: Vec<usize> = (0..total).filter(|cpu: &usize| !avoided.contains(cpu)).collect();
        if avoided.is_empty() || allowed.is_empty() {
            return None;
        }

        // SAFETY: `cpu_set_t` is plain data, zeroed then filled through the libc
        // helpers, and `sched_setaffinity(0, ..)` changes only the calling thread.
        let result: i32 = unsafe {
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            libc::CPU_ZERO(&mut set);
            for cpu in &allowed {
                libc::CPU_SET(*cpu, &mut set);
            }
            libc::sched_setaffinity(0, std::mem::size_of::<libc::cpu_set_t>(), &set)
        };

        (result == 0).then_some(allowed)
    }

    /// The CPUs servicing the interrupts that `table` (the text of
    /// `/proc/interrupts`) attributes to a GPU driver, sorted and deduplicated.
    ///
    /// `affinity_of` returns the `effective_affinity_list` of an interrupt
    /// number, such as `"3"` or `"0-1,4"`.
    pub(super) fn gpu_irq_cpus(table: &str, affinity_of: impl Fn(&str) -> Option<String>) -> Vec<usize> {
        let mut cpus: Vec<usize> = Vec::new();

        for line in table.lines() {
            let is_gpu: bool = line
                .split_whitespace()
                .any(|word: &str| GPU_DRIVERS.contains(&word.to_ascii_lowercase().as_str()));
            if !is_gpu {
                continue;
            }

            let Some(list) = line.split(':').next().map(str::trim).and_then(&affinity_of) else {
                continue;
            };

            for part in list.trim().split(',') {
                let bounds: Vec<usize> = part.split('-').filter_map(|v: &str| v.parse::<usize>().ok()).collect();
                match bounds[..] {
                    [single] => cpus.push(single),
                    [first, last] => cpus.extend(first..=last),
                    _ => {}
                }
            }
        }

        cpus.sort_unstable();
        cpus.dedup();
        cpus
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const TABLE: &str = "            CPU0       CPU1       CPU2       CPU3
 124:    4322585          0          0          0 IR-PCI-MSI-0000:00:14.0    0-edge      xhci_hcd
 143:          0          0          0     812345 IR-PCI-MSI-0000:01:00.0    0-edge      nvidia
 150:          0          0          0          0 IR-PCI-MSI-0000:02:00.0    0-edge      amdgpu
 160:          0          0          0          0 IR-PCI-MSI-0000:03:00.0    0-edge      nvme0q1";

        #[test]
        fn finds_only_gpu_driver_interrupts() {
            let cpus: Vec<usize> = gpu_irq_cpus(TABLE, |irq: &str| match irq {
                "143" => Some(String::from("3\n")),
                "150" => Some(String::from("0-1,3\n")),
                _ => panic!("asked for the affinity of non-GPU interrupt {irq}"),
            });

            assert_eq!(cpus, vec![0, 1, 3]);
        }

        #[test]
        fn unreadable_affinity_is_skipped() {
            assert!(gpu_irq_cpus(TABLE, |_: &str| None).is_empty());
        }
    }
}

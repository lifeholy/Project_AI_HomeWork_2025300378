//! CPU / 메모리 / 디스크 / 프로세스 스냅샷 수집 (sysinfo).

use sysinfo::{Disks, System};

#[derive(Clone, Debug)]
pub struct ProcRow {
    pub pid: u32,
    pub name: String,
    pub cpu: f64,
    pub mem_mb: f64,
    pub exe: String,
    pub cmd: String,
    pub start: u64,
    pub is_target: bool,
}

#[derive(Clone, Debug)]
pub struct DiskRow {
    pub mount: String,
    pub fs: String,
    pub total_gb: f64,
    pub used_gb: f64,
    pub free_gb: f64,
    pub pct: f64,
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub cpu: f64,
    pub ram: f64,
    pub disk: f64,
    pub proc_count: usize,
    pub procs: Vec<ProcRow>,
    pub missing_targets: Vec<String>,
    pub disks: Vec<DiskRow>,
}

pub struct Monitor {
    sys: System,
    cores: usize,
}

const GB: f64 = 1024.0 * 1024.0 * 1024.0;
const MB: f64 = 1024.0 * 1024.0;

impl Monitor {
    pub fn new() -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();
        let cores = sys.cpus().len().max(1);
        Self { sys, cores }
    }

    pub fn sample(&mut self, targets: &[String], drive: &str) -> Snapshot {
        self.sys.refresh_all();

        let cpu = (self.sys.global_cpu_info().cpu_usage() as f64).clamp(0.0, 100.0);
        let total_mem = self.sys.total_memory() as f64;
        let used_mem = self.sys.used_memory() as f64;
        let ram = if total_mem > 0.0 { used_mem / total_mem * 100.0 } else { 0.0 };

        let disks = Disks::new_with_refreshed_list();
        let disk_rows: Vec<DiskRow> = disks
            .list()
            .iter()
            .map(|d| {
                let total = d.total_space() as f64;
                let free = d.available_space() as f64;
                let used = (total - free).max(0.0);
                DiskRow {
                    mount: d.mount_point().to_string_lossy().to_string(),
                    fs: d.file_system().to_string_lossy().to_string(),
                    total_gb: total / GB,
                    used_gb: used / GB,
                    free_gb: free / GB,
                    pct: if total > 0.0 { used / total * 100.0 } else { 0.0 },
                }
            })
            .collect();
        let disk = disk_rows
            .iter()
            .find(|d| d.mount.eq_ignore_ascii_case(drive))
            .map(|d| d.pct)
            .unwrap_or(0.0);

        let lower_targets: Vec<String> = targets.iter().map(|t| t.to_lowercase()).collect();
        let mut found = vec![false; targets.len()];
        let mut procs = Vec::with_capacity(self.sys.processes().len());
        for (pid, p) in self.sys.processes() {
            let name = p.name().to_string_lossy().to_string();
            let lower = name.to_lowercase();
            let mut is_target = false;
            for (i, t) in lower_targets.iter().enumerate() {
                if *t == lower {
                    is_target = true;
                    found[i] = true;
                }
            }
            procs.push(ProcRow {
                pid: pid.as_u32(),
                name,
                cpu: (p.cpu_usage() as f64 / self.cores as f64 * 10.0).round() / 10.0,
                mem_mb: (p.memory() as f64 / MB * 10.0).round() / 10.0,
                exe: p
                    .exe()
                    .map(|e| e.to_string_lossy().to_string())
                    .unwrap_or_default(),
                cmd: p
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .collect::<Vec<_>>()
                    .join(" "),
                start: p.start_time(),
                is_target,
            });
        }
        let missing_targets = targets
            .iter()
            .zip(found.iter())
            .filter(|(_, f)| !**f)
            .map(|(t, _)| t.clone())
            .collect();

        Snapshot {
            cpu,
            ram,
            disk,
            proc_count: procs.len(),
            procs,
            missing_targets,
            disks: disk_rows,
        }
    }
}

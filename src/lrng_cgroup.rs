pub enum Controller {
    Memory,
    Cpu,
    CpuSet,
    BlkIo,
    Devices,
    Freezer,
    NetCls,
}

impl Controller {
    fn as_str(&self) -> &'static str {
        match self {
            Controller::Memory => "memory",
            Controller::Cpu => "cpu",
            Controller::CpuSet => "cpuset",
            Controller::BlkIo => "blkio",
            Controller::Devices => "devices",
            Controller::Freezer => "freezer",
            Controller::NetCls => "net_cls",
        }
    }
}

#[derive(Debug)]
pub struct CgroupManager {
    cgroup_root: std::path::PathBuf,
    cgroup_version: CgroupVersion,
}

#[derive(Debug, Clone)]
pub enum CgroupVersion {
    V1,
    V2,
}

#[derive(Debug)]
pub struct Cgroup {
    name: String,
    path: std::path::PathBuf,
    manager: CgroupManager,
}


#[derive(Debug, Default)]
pub struct MemoryStats {
    limit_in_bytes: Option<u64>,
    usage_in_bytes: u64,
    max_usage_in_bytes: u64,
    failcnt: u64,
}

#[derive(Debug, Default)]
pub struct CpuStats {
    shares: Option<u64>,
    quota: Option<i64>,
    period: Option<u64>,
    usage_ns: u64,
}

impl CgroupManager {
    /// Create a new cgroup manager, auto-detecting cgroup version
    pub fn new() -> std::io::Result<Self> {
        let cgroup_root = std::path::PathBuf::from("/sys/fs/cgroup");

        let version = if cgroup_root.join("cgroup.controllers").exists() {
            CgroupVersion::V2
        } else {
            CgroupVersion::V1
        };

        Ok(CgroupManager {
            cgroup_root,
            cgroup_version: version
        })
    }

    /// Create new cgroup with explicit root path
    pub fn with_root<P: AsRef<std::path::Path>>(root: P) -> std::io::Result<Self> {
        let cgroup_root = root.as_ref().to_path_buf();

        let version = if cgroup_root.join("cgroup.controllers").exists() {
            CgroupVersion::V2
        } else {

            CgroupVersion::V1
        };

        Ok(CgroupManager {

            cgroup_root,
            cgroup_version: version,
        })
    }

    pub fn version(&self) -> &CgroupVersion {
        &self.cgroup_version
    }

    pub fn create_cgroup(&self, name: &str, controllers: &[Controller]) -> std::io::Result<Cgroup> {
        match self.cgroup_version {
            CgroupVersion::V1 => self.create_cgroup_v1(name, controllers),
            CgroupVersion::V2 => self.create_cgroup_v2(name, controllers),
        }
    }

    fn create_cgroup_v1(&self, name: &str, controllers: &[Controller]) -> std::io::Result<Cgroup> {
        for controller in controllers {
            let controller_path = self.cgroup_root.join(controller.as_str()).join(name);
            std::fs::create_dir_all(&controller_path)?;
        }

        let main_path = if !controllers.is_empty() {
            self.cgroup_root.join(controllers[0].as_str()).join(name)
        } else {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "At least one controller required for v1"));
        };

        Ok(Cgroup {
            name: name.to_string(),
            path: main_path,
            manager: CgroupManager {
                cgroup_root: self.cgroup_root.clone(),
                cgroup_version: self.cgroup_version.clone(),
            }
        })
    }

    fn create_cgroup_v2(&self, name: &str, controllers: &[Controller]) -> std::io::Result<Cgroup> {
        let cgroup_path = self.cgroup_root.join(name);
        std::fs::create_dir_all(&cgroup_path)?;

        if !controllers.is_empty() {
            let controllers_str = controllers.iter()
                .map(|c| format!("+{}", c.as_str()))
                .collect::<Vec<_>>()
                .join(" ");

            let subtree_control_path = cgroup_path.join("cgroup.subtree_control");
            std::fs::write(&subtree_control_path, &controllers_str)?;
        }

        Ok (Cgroup {
            name: name.to_string(),
            path: cgroup_path,
            manager: CgroupManager {
                cgroup_root: self.cgroup_root.clone(),
                cgroup_version: self.cgroup_version.clone(),
            }
        })
    }

    pub fn get_cgroup(&self, name: &str, controller: Option<Controller>) -> std::io::Result<Cgroup> {
        let path = match (&self.cgroup_version, controller) {
            (CgroupVersion::V1, Some(ctrl)) => self.cgroup_root.join(ctrl.as_str()).join(name),
            (CgroupVersion::V1, None) => {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Controller required for v1"));
            },
            (CgroupVersion::V2, _) => self.cgroup_root.join(name),
         };

        if !path.exists() {
            return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "Cgroup not found"));
        }

        Ok(Cgroup {
            name: name.to_string(),
            path,
            manager: CgroupManager {
                cgroup_root: self.cgroup_root.clone(),
                cgroup_version: self.cgroup_version.clone(),
            }
        })
    }

    pub fn list_cgroups(&self, controller: Option<Controller>) -> std::io::Result<Vec<String>> {
        let search_path = match (&self.cgroup_version, controller) {
            (CgroupVersion::V1, Some(ctrl)) => self.cgroup_root.join(ctrl.as_str()),
            (CgroupVersion::V1, None) => {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Controller required for v1"));
            }
            (CgroupVersion::V2, _) => self.cgroup_root.clone()
        };

        let mut cgroups = Vec::new();
        self.collect_cgroups(&search_path, "", &mut cgroups)?;

        Ok(cgroups)
    }

    fn collect_cgroups(&self, path: &std::path::Path, prefix: &str, cgroups: &mut Vec<String>) -> std::io::Result<()> {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_dir() {
                let name = entry.file_name(); 
                let name = name.to_string_lossy();
                let full_name = if prefix.is_empty() {
                    name.to_string()
                } else {
                    format!("{}/{}", prefix, name)
                };

                cgroups.push(full_name.clone());
                self.collect_cgroups(&entry_path, &full_name, cgroups)?;
            }
        }

        Ok(())
    }
}

impl Cgroup {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn add_process(&self, pid: u32) -> std::io::Result<()> {
        let procs_file = match self.manager.cgroup_version {
            CgroupVersion::V1 => self.path.join("cgroup.procs"),
            CgroupVersion::V2 => self.path.join("cgroup.procs")
        };

        std::fs::write(&procs_file, pid.to_string())?;
        Ok(())
    }

    pub fn add_current_process(&self) -> std::io::Result<()> {
        let pid = std::process::id();
        self.add_process(pid)
    }

    pub fn get_processes(&self) -> std::io::Result<Vec<u32>> {
        let procs_file = self.path.join("cgroup.procs");
        let content = std::fs::read_to_string(&procs_file)?;

        let mut pids = Vec::new();
        for line in content.lines() {
            if let Ok(pid) = line.trim().parse::<u32>() {
                pids.push(pid);
            }
        }

        Ok(pids)
    }

    /// Set memory limit
    pub fn set_memory_limit(&self, limit_bytes: u64) -> std::io::Result<()> {
        let limit_file = match self.manager.cgroup_version {
            CgroupVersion::V1 => self.get_controller_path(Controller::Memory)?.join("memory.limit_in_bytes"),
            CgroupVersion::V2 => self.path.join("memory.max"),
        };

        std::fs::write(&limit_file, limit_bytes.to_string())?;
        Ok(())
    }


    /// Get memory statistics
    pub fn get_memory_stats(&self) -> std::io::Result<MemoryStats> {
        match self.manager.cgroup_version {
            CgroupVersion::V1 => self.get_memory_stats_v1(),
            CgroupVersion::V2 => self.get_memory_stats_v2(),
        }
    }

    fn get_memory_stats_v1(&self) -> std::io::Result<MemoryStats> {
        let mem_path = self.get_controller_path(Controller::Memory)?;

        let mut stats = MemoryStats::default();


        // Read limit
        if let Ok(content) = std::fs::read_to_string(mem_path.join("memory.limit_in_bytes")) {
            if let Ok(limit) = content.trim().parse::<u64>() {
                stats.limit_in_bytes = Some(limit);
            }
        }

        // Read usage
        if let Ok(content) = std::fs::read_to_string(mem_path.join("memory.usage_in_bytes")) {

            if let Ok(usage) = content.trim().parse::<u64>() {
                stats.usage_in_bytes = usage;
            }

        }


        // Read max usage
        if let Ok(content) = std::fs::read_to_string(mem_path.join("memory.max_usage_in_bytes")) {
            if let Ok(max_usage) = content.trim().parse::<u64>() {
                stats.max_usage_in_bytes = max_usage;
            }
        }

        // Read failcnt
        if let Ok(content) = std::fs::read_to_string(mem_path.join("memory.failcnt")) {
            if let Ok(failcnt) = content.trim().parse::<u64>() {
                stats.failcnt = failcnt;
            }

        }


        Ok(stats)
    }


    fn get_memory_stats_v2(&self) -> std::io::Result<MemoryStats> {
        let mut stats = MemoryStats::default();

        // Read limit
        if let Ok(content) = std::fs::read_to_string(self.path.join("memory.max")) {
            let limit_str = content.trim();

            if limit_str != "max" {
                if let Ok(limit) = limit_str.parse::<u64>() {
                    stats.limit_in_bytes = Some(limit);
                }
            }
        }


        // Read current usage
        if let Ok(content) = std::fs::read_to_string(self.path.join("memory.current")) {

            if let Ok(usage) = content.trim().parse::<u64>() {
                stats.usage_in_bytes = usage;
            }

        }


        // Read detailed stats
        if let Ok(content) = std::fs::read_to_string(self.path.join("memory.stat")) {
            for line in content.lines() {
                if let Some((key, value)) = line.split_once(' ') {

                    match key {
                        "oom_kill" => {
                            if let Ok(count) = value.parse::<u64>() {
                                stats.failcnt = count;
                            }
                        }
                        _ => {}
                    }
                }
            }

        }


        Ok(stats)
    }

    /// Set CPU shares (relative weight)
    pub fn set_cpu_shares(&self, shares: u64) -> std::io::Result<()> {
        let shares_file = match self.manager.cgroup_version {
            CgroupVersion::V1 => self.get_controller_path(Controller::Cpu)?.join("cpu.shares"),
            CgroupVersion::V2 => self.path.join("cpu.weight"),
        };

        let value = match self.manager.cgroup_version {
            CgroupVersion::V1 => shares.to_string(),
            CgroupVersion::V2 => {
                // Convert from v1 shares (1024 default) to v2 weight (100 default)

                let weight = (shares * 100) / 1024;
                weight.to_string()
            }
        };


        std::fs::write(&shares_file, value)?;
        Ok(())
    }

    /// Set CPU quota (microseconds per period)
    pub fn set_cpu_quota(&self, quota_us: i64, period_us: u64) -> std::io::Result<()> {
        match self.manager.cgroup_version {
            CgroupVersion::V1 => {
                let cpu_path = self.get_controller_path(Controller::Cpu)?;
                std::fs::write(cpu_path.join("cpu.cfs_quota_us"), quota_us.to_string())?;

                std::fs::write(cpu_path.join("cpu.cfs_period_us"), period_us.to_string())?;
            }
            CgroupVersion::V2 => {
                let quota_str = if quota_us < 0 {
                    "max".to_string()
                } else {

                    format!("{} {}", quota_us, period_us)
                };
                std::fs::write(self.path.join("cpu.max"), quota_str)?;
            }
        }
        Ok(())
    }


    /// Get CPU statistics
    pub fn get_cpu_stats(&self) -> std::io::Result<CpuStats> {
        match self.manager.cgroup_version {
            CgroupVersion::V1 => self.get_cpu_stats_v1(),
            CgroupVersion::V2 => self.get_cpu_stats_v2(),
        }
    }

    fn get_cpu_stats_v1(&self) -> std::io::Result<CpuStats> {
        let cpu_path = self.get_controller_path(Controller::Cpu)?;
        let mut stats = CpuStats::default();

        // Read shares

        if let Ok(content) = std::fs::read_to_string(cpu_path.join("cpu.shares")) {

            if let Ok(shares) = content.trim().parse::<u64>() {
                stats.shares = Some(shares);

            }
        }

        // Read quota
        if let Ok(content) = std::fs::read_to_string(cpu_path.join("cpu.cfs_quota_us")) {
            if let Ok(quota) = content.trim().parse::<i64>() {
                stats.quota = Some(quota);
            }

        }


        // Read period
        if let Ok(content) = std::fs::read_to_string(cpu_path.join("cpu.cfs_period_us")) {
            if let Ok(period) = content.trim().parse::<u64>() {
                stats.period = Some(period);
            }
        }

        // Read usage
        if let Ok(content) = std::fs::read_to_string(cpu_path.join("cpuacct.usage")) {

            if let Ok(usage) = content.trim().parse::<u64>() {
                stats.usage_ns = usage;

            }
        }

        Ok(stats)

    }

    fn get_cpu_stats_v2(&self) -> std::io::Result<CpuStats> {
        let mut stats = CpuStats::default();

        // Read weight (convert to shares)
        if let Ok(content) = std::fs::read_to_string(self.path.join("cpu.weight")) {
            if let Ok(weight) = content.trim().parse::<u64>() {
                // Convert from v2 weight (100 default) to v1 shares (1024 default)
                let shares = (weight * 1024) / 100;

                stats.shares = Some(shares);
            }
        }


        // Read quota
        if let Ok(content) = std::fs::read_to_string(self.path.join("cpu.max")) {
            let parts: Vec<&str> = content.trim().split_whitespace().collect();
            if parts.len() == 2 {
                if parts[0] != "max" {
                    if let Ok(quota) = parts[0].parse::<i64>() {

                        stats.quota = Some(quota);

                    }
                }
                if let Ok(period) = parts[1].parse::<u64>() {

                    stats.period = Some(period);
                }
            }
        }

        // Read usage
        if let Ok(content) = std::fs::read_to_string(self.path.join("cpu.stat")) {
            for line in content.lines() {
                if let Some((key, value)) = line.split_once(' ') {

                    if key == "usage_usec" {
                        if let Ok(usage_us) = value.parse::<u64>() {
                            stats.usage_ns = usage_us * 1000; // Convert to nanoseconds
                        }

                    }
                }
            }
        }

        Ok(stats)
    }

    /// Freeze all processes in this cgroup
    pub fn freeze(&self) -> std::io::Result<()> {
        let freeze_file = match self.manager.cgroup_version {
            CgroupVersion::V1 => self.get_controller_path(Controller::Freezer)?.join("freezer.state"),
            CgroupVersion::V2 => self.path.join("cgroup.freeze"),
        };

        let freeze_value = match self.manager.cgroup_version {
            CgroupVersion::V1 => "FROZEN",
            CgroupVersion::V2 => "1",
        };

        std::fs::write(&freeze_file, freeze_value)?;
        Ok(())
    }

    /// Unfreeze all processes in this cgroup
    pub fn unfreeze(&self) -> std::io::Result<()> {
        let freeze_file = match self.manager.cgroup_version {
            CgroupVersion::V1 => self.get_controller_path(Controller::Freezer)?.join("freezer.state"),
            CgroupVersion::V2 => self.path.join("cgroup.freeze"),
        };

        let unfreeze_value = match self.manager.cgroup_version {

            CgroupVersion::V1 => "THAWED",
            CgroupVersion::V2 => "0",
        };

        std::fs::write(&freeze_file, unfreeze_value)?;

        Ok(())
    }

    /// Delete this cgroup

    pub fn delete(&self) -> std::io::Result<()> {
        // First, make sure no processes are in the cgroup
        let procs = self.get_processes()?;
        if !procs.is_empty() {

            return Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "Cannot delete cgroup with active processes",
            ));
        }


        match self.manager.cgroup_version {
            CgroupVersion::V1 => {
                // For v1, we need to remove from all controller hierarchies
                // This is simplified - in reality you'd track which controllers were used
                for controller in [Controller::Memory, Controller::Cpu, Controller::CpuSet, 
                                   Controller::BlkIo, Controller::Devices, Controller::Freezer] {
                    let controller_path = self.manager.cgroup_root.join(controller.as_str()).join(&self.name);
                    if controller_path.exists() {
                        std::fs::remove_dir(&controller_path)?;
                    }
                }
            }
            CgroupVersion::V2 => {
                std::fs::remove_dir(&self.path)?;
            }
        }
        Ok(())

    }

    // Helper method to get controller-specific path for v1
    fn get_controller_path(&self, controller: Controller) -> std::io::Result<std::path::PathBuf> {

        match self.manager.cgroup_version {

            CgroupVersion::V1 => {
                Ok(self.manager.cgroup_root.join(controller.as_str()).join(&self.name))
            }
            CgroupVersion::V2 => Ok(self.path.clone()),

        }
    }
}

/// Example usage and demonstrations
pub mod examples {
    use super::*;
    use std::thread;
    use std::time::Duration;

    /// Create a memory-limited cgroup and add the current process
    pub fn memory_limit_example() -> std::io::Result<()> {
        println!("=== Memory Limit Example ===");
        

        let manager = CgroupManager::new()?;
        println!("Using cgroups {:?}", manager.version());

        // Create a cgroup with memory controller
        let cgroup = manager.create_cgroup("memory_test", &[Controller::Memory])?;

        println!("Created cgroup: {}", cgroup.name());

        // Set memory limit to 100MB
        cgroup.set_memory_limit(100 * 1024 * 1024)?;
        println!("Set memory limit to 100MB");


        // Add current process
        cgroup.add_current_process()?;
        println!("Added current process to cgroup");


        // Get and display memory stats
        let stats = cgroup.get_memory_stats()?;
        println!("Memory stats: {:?}", stats);

        // Clean up
        // Note: You'd need to move the process out first in a real scenario
        println!("Example completed (manual cleanup required)");

        
        Ok(())
    }

    /// Create a CPU-limited cgroup
    pub fn cpu_limit_example() -> std::io::Result<()> {
        println!("=== CPU Limit Example ===");
        
        let manager = CgroupManager::new()?;
        

        // Create a cgroup with CPU controller
        let cgroup = manager.create_cgroup("cpu_test", &[Controller::Cpu])?;

        println!("Created cgroup: {}", cgroup.name());

        // Set CPU shares (relative to other cgroups)
        cgroup.set_cpu_shares(512)?; // Half the default priority
        println!("Set CPU shares to 512");


        // Set CPU quota: 50ms every 100ms (50% of one CPU)
        cgroup.set_cpu_quota(50_000, 100_000)?;
        println!("Set CPU quota to 50% of one CPU");

        // Get and display CPU stats
        let stats = cgroup.get_cpu_stats()?;
        println!("CPU stats: {:?}", stats);

        Ok(())
    }


    /// Demonstrate process freezing

    pub fn freeze_example() -> std::io::Result<()> {
        println!("=== Freeze Example ===");
        
        let manager = CgroupManager::new()?;
        
        // Create a cgroup with freezer controller
        let cgroup = manager.create_cgroup("freeze_test", &[Controller::Freezer])?;
        println!("Created cgroup: {}", cgroup.name());

        // In a real scenario, you'd add a different process here
        println!("Would freeze processes here...");
        
        // Demonstrate freeze/unfreeze (commented out to avoid freezing the example)
        // cgroup.freeze()?;
        // println!("Frozen all processes in cgroup");
        // 
        // thread::sleep(Duration::from_secs(2));
        // 
        // cgroup.unfreeze()?;
        // println!("Unfrozen all processes in cgroup");

        Ok(())
    }

    /// List all cgroups
    pub fn list_cgroups_example() -> std::io::Result<()> {
        println!("=== List cgroups Example ===");
        

        let manager = CgroupManager::new()?;
        
        match manager.version() {

            CgroupVersion::V1 => {
                println!("Memory cgroups:");
                let cgroups = manager.list_cgroups(Some(Controller::Memory))?;
                for cgroup in cgroups.iter().take(10) { // Limit output
                    println!("  {}", cgroup);
                }
            }
            CgroupVersion::V2 => {
                println!("All cgroups:");
                let cgroups = manager.list_cgroups(None)?;
                for cgroup in cgroups.iter().take(10) { // Limit output
                    println!("  {}", cgroup);
                }

            }
        }
        
        Ok(())
    }


    /// Complete workflow example
    pub fn complete_workflow() -> std::io::Result<()> {
        println!("=== Complete Workflow Example ===");
        
        let manager = CgroupManager::new()?;
        println!("Initialized cgroup manager (version: {:?})", manager.version());

        // Create a cgroup with multiple controllers
        let controllers = match manager.version() {
            CgroupVersion::V1 => vec![Controller::Memory, Controller::Cpu],

            CgroupVersion::V2 => vec![Controller::Memory, Controller::Cpu],
        };

        let cgroup = manager.create_cgroup("demo_cgroup", &controllers)?;
        println!("Created cgroup: {}", cgroup.name());

        // Configure limits

        cgroup.set_memory_limit(50 * 1024 * 1024)?; // 50MB
        cgroup.set_cpu_shares(256)?; // Low priority
        println!("Configured resource limits");

        // Get current stats
        let mem_stats = cgroup.get_memory_stats()?;
        let cpu_stats = cgroup.get_cpu_stats()?;
        println!("Memory stats: {:?}", mem_stats);
        println!("CPU stats: {:?}", cpu_stats);


        // List processes (should be empty since we didn't add any)
        let processes = cgroup.get_processes()?;

        println!("Processes in cgroup: {:?}", processes);

        // Clean up would happen here in a real application
        println!("Workflow completed (cleanup required)");
        
        Ok(())
    }

}

fn main() -> std::io::Result<()> {
    println!("Rust cgroups Tutorial\n");
    
    // Check if we have the necessary permissions
    if !std::path::Path::new("/sys/fs/cgroup").exists() {
        eprintln!("Error: /sys/fs/cgroup not found. This tutorial requires Linux with cgroups support.");
        eprintln!("Note: You may need root privileges to create and manage cgroups.");
        return Ok(());
    }

    // Run examples
    if let Err(e) = examples::list_cgroups_example() {
        eprintln!("List cgroups example failed: {}", e);
    }
    
    println!();


    if let Err(e) = examples::complete_workflow() {
        eprintln!("Complete workflow example failed: {}", e);
        eprintln!("Note: This might fail without root privileges");
    }

    println!("\n=== Tutorial completed ===");
    println!("This tutorial demonstrated:");
    println!("1. Auto-detection of cgroups v1/v2");
    println!("2. Creating and managing cgroups");
    println!("3. Setting memory and CPU limits");
    println!("4. Process management within cgroups");
    println!("5. Reading resource usage statistics");

    println!("6. Freezing/unfreezing processes");
    println!("\nTo run the examples with actual cgroup creation, you'll need root privileges.");
    
    Ok(())
}

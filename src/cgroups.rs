use std::io::Write as _;

use crate::ActionResult;

pub struct CgroupManager {
    pub cgroup_path: String,
}

impl CgroupManager {
    pub fn new(container_id: &str) -> Self {
        CgroupManager {
            cgroup_path: format!("/sys/fs/cgroup/woody/{}", container_id)
        }
    }

    pub fn create(&self) -> ActionResult {
        std::fs::create_dir_all(&self.cgroup_path)?;
        Ok(())
    }

    // This enables controllers for the cgroup we are about to use.
    // It must be run before setting limits.
    pub fn enable_controllers(&self) -> ActionResult {
        // You enable controllers from the parent directory.
        // NOTE: This assumes "/sys/fs/cgroup/woody" already exists.
        // Your setup script might need to run `mkdir /sys/fs/cgroup/woody` once.
        let subtree_path = "/sys/fs/cgroup/woody/cgroup.subtree_control";
        // The `+` enables the controller for child cgroups.
        std::fs::write(subtree_path, "+pids +memory")?;
        Ok(())
    }

    // Crucial for preventing "fork: Cannot allocate memory"
    pub fn set_pid_limit(&self, limit: u32) -> ActionResult {
        let path = format!("{}/pids.max", self.cgroup_path);
        std::fs::write(path, limit.to_string())?;
        Ok(())
    }


    pub fn set_memory_limit(&self, limit: u64) -> ActionResult {
        let memory_limit_path = format!("{}/memory.max", self.cgroup_path);
        let mut file = std::fs::File::create(memory_limit_path)?;

        file.write_all(limit.to_string().as_bytes())?;
        Ok(())
    }

    pub fn add_process(&self, pid: nix::unistd::Pid) -> ActionResult {
        let procs_path = format!("{}/cgroup.procs", self.cgroup_path);
        let mut file = std::fs::File::create(procs_path)?;
        file.write_all(pid.to_string().as_bytes())?;

        Ok(())
    }

    pub fn destroy(&self) -> ActionResult {
        std::fs::remove_dir_all(&self.cgroup_path).ok();
        Ok(())
    }
}

// use nix::sys::prctl;
//
// fn drop_capabilities() -> Result<(), Box<dyn std::error::Error>> {
//     // Drop all capabilities except the essential ones
//
//     let keep_caps = vec![
//         // Add specific capabilities you want to keep
//     ];
//
//
//     // This is a simplified version - you'd want to use libcap-ng or similar
//     // for proper capability management
//
//
//     Ok(())
// }

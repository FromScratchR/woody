use std::io::Write as _;

use crate::ActionResult;

struct CgroupManager {
    cgroup_path: String,
}

impl CgroupManager {
    fn new(container_id: &str) -> Self {
        CgroupManager {
            cgroup_path: format!("/sys/fs/cgroup/myruntime/{}", container_id)
        }
    }

    fn create(&self) -> ActionResult {
        std::fs::create_dir_all(&self.cgroup_path)?;
        Ok(())
    }

    fn set_memory_limit(&self, limit: u64) -> ActionResult {
        let memory_limit_path = format!("{}/memory.max", self.cgroup_path);
        let mut file = std::fs::File::create(memory_limit_path)?;

        file.write_all(limit.to_string().as_bytes())?;
        Ok(())
    }

    fn add_process(&self, pid: nix::unistd::Pid) -> ActionResult {
        let procs_path = format!("{}/cgroup.procs", self.cgroup_path);
        let mut file = std::fs::File::create(procs_path)?;
        file.write_all(pid.to_string().as_bytes())?;

        Ok(())
    }

    fn destroy(&self) -> ActionResult {
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

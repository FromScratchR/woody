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
    usage_in_byts: u64,
    max_usage_in_bytes: u64,
    failcnt: u64,
}

#[derive(Debug, Default)]
pub struct CpuStats {
    shares: Option<u64>,
    quote: Option<i64>,
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
}

mod cgroups;
mod lrng_cgroup;
mod container;

// use crate::lrng_cgroup;
use crate::container::{Container, ContainerConfig};

pub type ActionResult = Result<(), Box<dyn std::error::Error>>;

fn main() {
    let config = ContainerConfig {
        command: vec!["/bin/bash".to_string()],
        args: vec![],
        rootfs: "./container/".to_string(),
    };

    let container = Container::new(config);
    container.run();
}


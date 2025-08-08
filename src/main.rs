mod cgroups;
mod container;
use crate::container::{Container, ContainerConfig};

pub type ActionResult = Result<(), Box<dyn std::error::Error>>;

fn main() {
    let config = ContainerConfig {
        command: vec!["/bin/ls".to_string()],
        args: vec!["-la".to_string()],
        rootfs: "~/Progs/woody/container/".to_string(),
    };

    let container = Container::new(config);
    container.run();
}


pub mod mem_store;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub name: String,
    pub description: String,
    pub f_type: String,
}

pub trait FaultStore {
    fn store(&mut self, key: &str, fault: Fault);
    fn get_by_command(&self, cmd: &str) -> Option<Fault>;
}

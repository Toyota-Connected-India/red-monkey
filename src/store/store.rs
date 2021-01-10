pub struct Fault<'a> {
    name: &'a str,
    description: &'a str,
    f_type: &'a str,
}

pub trait FaultStore<'a> {
    fn Store(self, fault: Fault);
    fn GetAll(self) -> Vec<Fault<'a>>;
    fn GetByCommand(self, cmd: &str) -> Option<Fault>;
}

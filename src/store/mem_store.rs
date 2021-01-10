use crate::store::store::*;
use chashmap;

struct MemStore<'a> {
    store: chashmap::CHashMap<&'a str, Fault<'a>>,
}

pub fn new() -> Box<dyn FaultStore<'static>> {
    Box::new(MemStore {
        store: chashmap::CHashMap::new(),
    })
}

impl MemStore {
    fn Store(&mut self, key: &str, fault: Fault) {
        self.store.insert(key, fault);
    }

    fn GetAll(self) -> Vec<Fault<'static>> {
        vec![Fault {
            name: "sample fault",
            f_type: "delay",
        }]
    }

    fn GetByCommand(self, cmd: &str) -> Option<Fault> {
        match self.store.get(cmd) {
            Some(val) => Some(Fault {
                name: "sample fault".to_string(),
            }),
            None => None,
        }
    }
}

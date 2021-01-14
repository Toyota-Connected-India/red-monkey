use crate::store::*;

struct MemStore {
    store: chashmap::CHashMap<String, Fault>,
}

pub fn new() -> Box<dyn FaultStore> {
    Box::new(MemStore {
        store: chashmap::CHashMap::new(),
    })
}

impl FaultStore for MemStore {
    fn store(&mut self, key: &str, fault: Fault) {
        self.store.insert(key.to_string(), fault);
    }

    fn get_by_command(&self, cmd: &str) -> Option<Fault> {
        match self.store.get(cmd) {
            Some(val) => Some((*val).clone()),
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::store::mem_store;
    use crate::store::*;

    #[test]
    fn test_mem_store() {
        let mut mem_store = mem_store::new();
        let key = "delay_10_seconds";
        let fault = Fault {
            name: "delay 10 seconds".to_string(),
            description: "inject a delay of 10 seconds".to_string(),
            f_type: "inject_delay".to_string(),
        };

        mem_store.store(key, fault.clone());

        match mem_store.get_by_command(key) {
            Some(actual_fault) => {
                assert_eq!(fault, actual_fault);
            }
            None => {}
        };
    }
}

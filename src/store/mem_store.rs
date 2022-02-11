use crate::store::fault_store::{Fault, FaultStore, StoreError, DB};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error};

/// MemStore is an in-memory store implementation of FaultStore
#[derive(Debug, Clone)]
pub struct MemStore {
    store: chashmap::CHashMap<String, Fault>,
}

impl MemStore {
    pub fn new_db() -> DB {
        Arc::new(RwLock::new(Box::new(MemStore {
            store: chashmap::CHashMap::new(),
        })))
    }
}

impl FaultStore for MemStore {
    fn store(&self, fault_name: &str, fault: &Fault) -> Result<bool, StoreError> {
        match self.store.insert(fault_name.to_string(), fault.clone()) {
            None => {
                debug!("Fault {} stored in memory", fault.name);
                Ok(true)
            }
            Some(val) => {
                debug!("Fault {} is replaced by the latest config", val.name);
                Ok(true)
            }
        }
    }

    fn get_by_fault_name(&self, fault_name: &str) -> Result<Fault, StoreError> {
        match self.store.get(fault_name) {
            Some(val) => Ok(val.clone()),
            None => Err(StoreError::new(
                format!("Fault {} not found", fault_name).as_str(),
            )),
        }
    }

    fn get_by_redis_cmd(&self, redis_cmd: &str) -> Option<Fault> {
        let faults = match self.get_all_faults() {
            Ok(faults) => faults,
            Err(e) => {
                error!("error fetching all faults: {:?}", e);
                return None;
            }
        };

        for fault in faults {
            if redis_cmd.to_lowercase() == fault.command.to_lowercase() {
                return Some(fault);
            }
        }

        None
    }

    fn get_all_faults(&self) -> Result<Vec<Fault>, StoreError> {
        let mut faults = Vec::new();
        for (_, value) in self.store.clone() {
            faults.push(value);
        }

        Ok(faults)
    }

    fn delete_fault(&self, fault_name: &str) -> Result<bool, StoreError> {
        match self.store.remove(fault_name) {
            None => Ok(false),
            Some(fault) => {
                debug!("Delete fault {}", fault.name);
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::store::fault_store::*;
    use crate::store::mem_store;
    use chrono::{Duration, Utc};

    #[tokio::test]
    async fn test_store() {
        let mem_store = mem_store::MemStore::new_db();

        let fault = get_mock_fault();
        match mem_store.write().await.store(fault.name.as_str(), &fault) {
            Ok(val) => {
                assert_eq!(true, val);
            }
            Err(e) => {
                panic!("store test failed {}", e);
            }
        };
    }

    #[tokio::test]
    async fn test_duplicate_store() {
        let mem_store = mem_store::MemStore::new_db();

        let mut fault = get_mock_fault();
        match mem_store.write().await.store(fault.name.as_str(), &fault) {
            Ok(val) => {
                assert_eq!(true, val);
            }
            Err(e) => {
                panic!("store failed {}", e);
            }
        };

        fault.command = "GET".to_string();

        match mem_store.write().await.store(fault.name.as_str(), &fault) {
            Ok(val) => {
                assert_eq!(true, val);
            }
            Err(e) => {
                panic!("store fault test failed {}", e);
            }
        };

        match mem_store
            .read()
            .await
            .get_by_fault_name(fault.name.as_str())
        {
            Ok(fault) => {
                assert_eq!(fault.command, "GET");
            }
            Err(e) => {
                panic!("{}", e);
            }
        };
    }

    #[tokio::test]
    async fn test_get_fault_by_name() {
        let mem_store = mem_store::MemStore::new_db();

        let fault = get_mock_fault();
        match mem_store.write().await.store(fault.name.as_str(), &fault) {
            Ok(_) => {}
            Err(e) => {
                panic!("store fault test failed {}", e);
            }
        }

        match mem_store
            .read()
            .await
            .get_by_fault_name(fault.name.as_str())
        {
            Ok(fault) => {
                assert_eq!(fault, get_mock_fault());
            }
            Err(e) => {
                panic!("get_by_fault_name test failed {}", e);
            }
        };
    }

    #[tokio::test]
    async fn test_get_all_faults() {
        let mem_store = mem_store::MemStore::new_db();

        let mock_faults = vec![
            Fault {
                name: "delay 10 milliseconds".to_string(),
                description: Some("inject a delay of 10 milliseconds".to_string()),
                fault_type: "delay".to_string(),
                duration: Some(20),
                error_msg: None,
                command: "SET".to_string(),
                last_modified: Some(Utc::now()),
            },
            Fault {
                name: "SET Error".to_string(),
                description: Some("inject set error".to_string()),
                fault_type: "error".to_string(),
                duration: None,
                error_msg: Some("SET ERROR".to_string()),
                command: "SET".to_string(),
                last_modified: Some(Utc::now() + Duration::minutes(1)),
            },
        ];

        for mock_fault in &mock_faults {
            match mem_store
                .write()
                .await
                .store(mock_fault.name.as_str(), &mock_fault)
            {
                Ok(_) => {}
                Err(e) => {
                    panic!("store fault test failed {}", e);
                }
            };
        }

        match mem_store.read().await.get_all_faults() {
            Ok(faults) => {
                let n = mock_faults.len();
                assert_eq!(faults.len(), n);

                let mut faults = faults.clone();
                faults.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));

                for (i, fault) in faults.into_iter().enumerate() {
                    assert_eq!(&fault.name, &mock_faults[n - i - 1].name);
                }
            }
            Err(e) => {
                panic!("get_all_faults test failed {}", e);
            }
        };
    }

    #[tokio::test]
    async fn test_delete_fault() {
        let mem_store = mem_store::MemStore::new_db();

        let fault = get_mock_fault();
        match mem_store.write().await.store(fault.name.as_str(), &fault) {
            Ok(_) => {}
            Err(e) => {
                panic!("store fault test failed {}", e);
            }
        }

        match mem_store.write().await.delete_fault(fault.name.as_str()) {
            Ok(is_deleted) => {
                assert_eq!(is_deleted, true);
            }
            Err(e) => {
                panic!("delete fault test failed: {}", e);
            }
        };
    }

    #[tokio::test]
    async fn test_delete_invalid_fault() {
        let mem_store = mem_store::MemStore::new_db();

        match mem_store.write().await.delete_fault("invalid_fault") {
            Ok(is_deleted) => {
                assert_eq!(is_deleted, false);
            }
            Err(e) => {
                panic!("delete fault test failed: {}", e);
            }
        };
    }

    fn get_mock_fault() -> Fault {
        Fault {
            name: "delay 10 millimilliseconds".to_string(),
            description: Some("inject a delay of 10 milliseconds".to_string()),
            fault_type: "delay".to_string(),
            duration: Some(20),
            error_msg: None,
            command: "SET".to_string(),
            last_modified: None,
        }
    }
}

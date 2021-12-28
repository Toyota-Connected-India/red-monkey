use crate::proxy::{connection::Error, resp_util};
use crate::store::fault_store::{Fault, FaultStore, DB, DELAY_FAULT, ERROR_FAULT};
use log::{debug, error, info};
use std::str;
use std::sync::{Arc, RwLock};
use std::{thread, time};

#[derive(Clone)]
pub struct Faulter {
    fault_store: DB,
}

#[derive(Clone, Debug)]
pub enum FaulterValue {
    Value(Vec<u8>),
    Null,
}

impl Faulter {
    pub fn new(fault_store: Arc<RwLock<Box<dyn FaultStore + Sync + Send>>>) -> Self {
        Faulter { fault_store }
    }

    pub fn apply_fault(&self, req_body: &str) -> Result<FaulterValue, Error> {
        let redis_command: String;
        let result = resp_util::decode(req_body);

        match result {
            Ok(val) => match resp_util::fetch_redis_command(val) {
                Ok(command) => {
                    debug!("redis command: {}", command);
                    redis_command = command;
                }
                Err(err) => {
                    error!("error fetching redis command from req: {:?}", err);
                    return Err(err);
                }
            },

            Err(err) => {
                error!("error decoding request body: {:?}", err);
                return Err(err);
            }
        };

        let fault_store = self.fault_store.read().unwrap();
        let fault_config = fault_store.get_by_redis_cmd(redis_command.as_str());

        let fault = match fault_config {
            Some(fault) => fault,
            None => {
                return Ok(FaulterValue::Null);
            }
        };

        match fault.fault_type.as_str() {
            DELAY_FAULT => {
                info!("applying delay fault: {:?}", fault);
                self.apply_delay_fault(fault.duration);
                Ok(FaulterValue::Null)
            }

            ERROR_FAULT => {
                info!("applying error fault: {:?}", fault);
                self.apply_error_fault(fault)
            }

            _ => Err(Box::new(FaulterErrors::UnsupportedFaultTypeError)),
        }
    }

    pub fn apply_delay_fault(&self, sleep_duration: Option<u64>) {
        debug!("Sleep for {:?} seconds", sleep_duration);

        let sleep_duration = time::Duration::from_secs(sleep_duration.unwrap());
        thread::sleep(sleep_duration);

        debug!("Slept {:?} seconds", sleep_duration);
    }

    pub fn apply_error_fault(&self, fault: Fault) -> Result<FaulterValue, Error> {
        let encoded_err_msg = resp_util::encode_error_message(
            fault
                .error_msg
                .ok_or_else(|| Box::new(FaulterErrors::EncodeErrMsgError))?,
        )?;

        Ok(FaulterValue::Value(encoded_err_msg))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FaulterErrors {
    #[error("Error decoding request body to RESP values")]
    EncodeErrMsgError,
    #[error("Error as fault type is unsupported")]
    UnsupportedFaultTypeError,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::fault_store::DB;
    use crate::store::{self};
    use chrono::{Duration, Utc};
    use std::time::Instant;

    fn get_mock_fault_store() -> DB {
        let mock_faults = vec![
            Fault {
                name: "delay 10 milliseconds".to_string(),
                description: Some("inject a delay of 10 milliseconds".to_string()),
                fault_type: "delay".to_string(),
                duration: Some(2),
                error_msg: None,
                command: "GET".to_string(),
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

        let fault_store = store::mem_store::MemStore::new_db();

        for fault in mock_faults {
            fault_store
                .write()
                .unwrap()
                .store(fault.name.as_str(), &fault)
                .unwrap();
        }

        fault_store
    }

    #[test]
    fn test_error_fault() {
        let fault_store = get_mock_fault_store();
        let faulter = Faulter::new(fault_store);

        let res = faulter.apply_fault("*3\r\n$3\r\nset\r\n$4\r\nkey1\r\n$8\r\nvalue100\r\n");
        assert_eq!(res.is_ok(), true);

        let err_val = res.unwrap();
        match err_val {
            FaulterValue::Value(err_response) => {
                assert_eq!(str::from_utf8(&err_response).unwrap(), "-SET ERROR\r\n");
            }
            FaulterValue::Null => {
                panic!("err_val is not expected to be null");
            }
        }
    }

    #[test]
    fn test_delay_fault() {
        let fault_store = get_mock_fault_store();
        let faulter = Faulter::new(fault_store);

        let start = Instant::now();
        let res = faulter.apply_fault("*2\r\n$3\r\nget\r\n$4\r\nkey1\r\n");
        let duration = start.elapsed();
        println!("elapsed duration is: {:?}", duration.as_secs());

        assert_eq!(res.is_ok(), true);
        assert_eq!(duration.as_secs() >= 2, true);
    }
}

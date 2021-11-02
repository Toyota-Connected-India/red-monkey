use crate::proxy::resp_util;
use crate::store::fault_store::{Fault, FaultStore, DELAY_FAULT, ERROR_FAULT};
use log::{debug, error};
use std::string::ToString;
use std::{thread, time};

#[derive(Clone)]
pub struct Faulter {
    fault_store: Box<dyn FaultStore>,
}

type Error = Box<dyn ::std::error::Error>;

pub enum FaulterValue {
    Value(Vec<u8>),
    Null,
}

impl Faulter {
    pub fn new(fault_store: Box<dyn FaultStore>) -> Self {
        Faulter { fault_store }
    }

    pub fn apply_fault(&self, req_body: &str) -> Result<FaulterValue, Error> {
        let mut redis_command: String = "".to_string();
        let result = resp_util::decode(req_body);

        match result {
            Ok(val) => match resp_util::fetch_redis_command(val) {
                Ok(val) => {
                    redis_command = val;
                }
                Err(_) => {
                    todo!();
                }
            },

            Err(_) => {
                todo!();
            }
        };

        let faults = self.fault_store.get_all_faults().unwrap();
        debug!("faults: {:?}", faults);

        for fault in faults {
            if redis_command.to_lowercase() == fault.command.to_lowercase() {
                match fault.fault_type.as_str() {
                    DELAY_FAULT => {
                        self.apply_delay_fault(fault.duration);
                    }

                    ERROR_FAULT => {
                        self.apply_error_fault(fault)?;
                    }

                    _ => {
                        todo!();
                    }
                };
            }
        }

        Ok(FaulterValue::Null)
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
                .ok_or(Box::new(FaulterErrors::EncodeErrMsgError))?,
        )?;

        return Ok(FaulterValue::Value(encoded_err_msg));
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FaulterErrors {
    #[error("Error decoding request body to RESP values")]
    EncodeErrMsgError,
}

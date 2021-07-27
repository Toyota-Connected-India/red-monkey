use log::debug;
use std::{thread, time};

use crate::store::fault_store::FaultStore;

#[derive(Clone)]
pub struct Faulter {
    fault_store: Box<dyn FaultStore>,
}

impl Faulter {
    pub fn new(fault_store: Box<dyn FaultStore>) -> Self {
        Faulter { fault_store }
    }

    pub fn apply_delay_fault(&self, sleep_duration: Option<u64>) {
        debug!("Sleep for {:?} seconds", sleep_duration);

        // TODO: Don't use unwrap
        let sleep_duration = time::Duration::from_secs(sleep_duration.unwrap());
        thread::sleep(sleep_duration);

        debug!("Slept {:?} seconds", sleep_duration);
    }

    pub fn check_fault(&self, request_payload: &str) {
        match self.fault_store.get_all_faults() {
            // TODO: Handle the error.
            Err(_) => {}
            Ok(faults) => {
                for fault in faults {
                    if request_payload.contains(&fault.command) {
                        debug!("Command {:?} matched; applying fault", fault.command);

                        match fault.fault_type.as_str() {
                            // TODO: Use string constant
                            "delay" => {
                                self.apply_delay_fault(fault.duration);
                            }

                            "error" => {}

                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;
use tokio::sync::RwLock;

pub type DB = Arc<RwLock<Box<dyn FaultStore + Send + Sync>>>;

pub const DELAY_FAULT: &str = "delay";
pub const ERROR_FAULT: &str = "error";

/// Fault represents fault configurations that can be applied on an incoming request
/// Two types of fault configurations are supported - `delay` and `error`
///
/// ## Example `delay` fault
///
/// ```
/// Fault {
///  name: "delay 10 seconds".to_string(),
///  description: Some("inject a delay of 10 milliseconds".to_string()),
///  fault_type: "delay".to_string(),
///  duration: Some(20),
///  error_msg: None,
///  command: "SET".to_string(),
/// }
/// ```
///
/// ## Example `error` fault
///
/// ```
/// Fault {
///  name: "SET Error".to_string(),
///  description: Some("inject set error".to_string()),
///  fault_type: "error".to_string(),
///  duration: None,
///  error_msg: Some("SET ERROR".to_string()),
///  command: "SET".to_string(),
/// }
/// ```
///
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fault {
    /// name represents the fault name that acts as the primary key in the store
    pub name: String,

    /// description provides the optional human-friendly description about the fault
    pub description: Option<String>,

    /// fault_type accepts one of the `delay`, `error` as the fault type value
    pub fault_type: String,

    /// In the event of `delay` fault, the duration of the delay in milliseconds will be set in
    /// this field  
    pub duration: Option<u64>,

    /// In the event of `error` fault, the error string is set in this field
    pub error_msg: Option<String>,

    /// command accepts any valid `redis` command
    pub command: String,

    // last_modified holds the timestamp at which the fault is created or last modified
    pub last_modified: Option<DateTime<Utc>>,
}

#[derive(Debug, PartialEq)]
pub enum FaultVariants {
    Delay,
    Error,
}

impl FaultVariants {
    pub fn as_str(&self) -> &'static str {
        match self {
            FaultVariants::Delay => "delay",
            FaultVariants::Error => "error",
        }
    }
}

/// A trait providing methods for pluggable data store
pub trait FaultStore: FaultStoreClone {
    /// Stores the fault in the store
    fn store(&self, key: &str, fault: &Fault) -> Result<bool, StoreError>;

    /// Fetch the fault by the given fault name from the store
    fn get_by_fault_name(&self, fault_name: &str) -> Result<Fault, StoreError>;

    /// Fetch all the faults from the store
    fn get_all_faults(&self) -> Result<Vec<Fault>, StoreError>;

    /// Fetch the fault that matches the redis command
    fn get_by_redis_cmd(&self, redis_cmd: &str) -> Option<Fault>;

    /// Delete the fault by the given fault name in the store
    fn delete_fault(&self, fault_name: &str) -> Result<bool, StoreError>;
}

pub trait FaultStoreClone {
    fn clone_box(&self) -> Box<dyn FaultStore>;
}

impl<T> FaultStoreClone for T
where
    T: FaultStore + 'static + Clone + Send + Sync,
{
    fn clone_box(&self) -> Box<dyn FaultStore> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn FaultStore> {
    fn clone(&self) -> Box<dyn FaultStore> {
        self.clone_box()
    }
}

/// StoreError is a representation of any data store related errors.
#[derive(Debug)]
pub struct StoreError {
    pub message: String,
}

impl StoreError {
    pub fn new(msg: &str) -> Self {
        StoreError {
            message: msg.to_string(),
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

use crate::store::fault_store::Fault;
use crate::store::fault_store::FaultStore;
use log::debug;
use rocket::State;
use rocket::{delete, get, post};
use rocket_contrib::json::Json;

#[post("/fault", format = "json", data = "<fault>")]
pub fn create_fault(fault: Json<Fault>, fault_store: State<Box<dyn FaultStore + Send + Sync>>) {
    debug!("create fault: fault name: {:?}", fault.name);

    match fault_store.store(&fault.name, &fault) {
        Ok(_) => {}
        Err(_) => {}
    };
}

#[get("/fault/<name>", format = "json")]
pub fn get_fault(
    name: String,
    fault_store: State<Box<dyn FaultStore + Send + Sync>>,
) -> Option<Json<Fault>> {
    debug!("get fault by name: {:?}", name);

    match fault_store.get_by_fault_name(name.as_str()) {
        Err(_) => None,
        Ok(fault) => Some(Json(fault)),
    }
}

#[get("/faults", format = "json")]
pub fn get_all_faults(fault_store: State<Box<dyn FaultStore + Send + Sync>>) -> Json<Vec<Fault>> {
    debug!("get all faults");
    let faults = fault_store.get_all_faults();

    match faults {
        Err(_) => Json(Vec::new()),
        Ok(faults) => Json(faults),
    }
}

#[delete("/fault/<fault_name>")]
pub fn delete_fault(fault_name: String, fault_store: State<Box<dyn FaultStore + Send + Sync>>) {
    debug!("delete fault: {}", fault_name);

    match fault_store.delete_fault(fault_name.as_str()) {
        Err(_) => {}
        Ok(_) => {
            debug!("Deleted fault: {:?}", fault_name);
        }
    }
}

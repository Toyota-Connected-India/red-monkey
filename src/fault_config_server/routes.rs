use crate::fault_config_server::handler::*;
use crate::store::fault_store::FaultStore;
use rocket::*;

pub fn run(fault_store: Box<dyn FaultStore + Send + Sync>) {
    rocket::ignite()
        .mount(
            "/",
            routes![create_fault, get_fault, get_all_faults, delete_fault],
        )
        .manage(fault_store)
        .launch();
}

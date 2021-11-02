use crate::server::handler::*;
use crate::store::fault_store::FaultStore;
use rocket::*;
use std::error::Error;

pub async fn run(fault_store: Box<dyn FaultStore + Send + Sync>) -> Result<(), Box<dyn Error>> {
    rocket::ignite()
        .mount(
            "/",
            routes![
                store_fault,
                get_fault,
                get_all_faults,
                delete_fault,
                delete_all_faults
            ],
        )
        .manage(fault_store)
        .launch();

    Ok(())
}

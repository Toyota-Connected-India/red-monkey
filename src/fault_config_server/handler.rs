use crate::store::fault_store::{Fault, DB, LOCK_ERROR_CODE};
use chrono::Utc;
use log::{debug, error, info};
use rocket;
use rocket::http::{ContentType, Status};
use rocket::request::Request;
use rocket::response::{self, Responder, Response};
use rocket::State;
use rocket::{delete, get, post};
use rocket_contrib::json::Json;
use std::io::Cursor;

// ServerErrorResponse represents the response payload for server error (Internal Server Error)
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerErrorResponse {
    pub error_code: String,
    pub error_msg: String,
}

impl ServerErrorResponse {
    fn new(error_code: String, error_msg: String) -> Self {
        ServerErrorResponse {
            error_code,
            error_msg,
        }
    }

    fn to_vec(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap()
    }
}

impl<'a> Responder<'a> for ServerErrorResponse {
    fn respond_to(self, _: &Request) -> response::Result<'a> {
        Response::build()
            .header(ContentType::JSON)
            .status(Status::InternalServerError)
            .sized_body(Cursor::new(self.to_vec()))
            .ok()
    }
}

// store_fault is the handler of POST /fault endpoint.
// On success, returns the response with Created (201) HTTP status.
// On failure, returns the response with Internal Server Error (500) HTTP status.
#[post("/fault", format = "json", data = "<fault>")]
pub fn store_fault(
    fault: Json<Fault>,
    fault_store: State<DB>,
) -> Result<Status, ServerErrorResponse> {
    info!("Create fault: fault name: {:?}", fault.name);

    let mut fault = fault.clone();
    fault.last_modified = Some(Utc::now());

    let fault_store = fault_store
        .write()
        .map_err(|err| ServerErrorResponse::new(LOCK_ERROR_CODE.to_string(), err.to_string()))?;

    match fault_store.store(&fault.name, &fault) {
        Ok(_) => {
            info!("Fault {} is created in the store", fault.name);
            Ok(Status::Created)
        }

        Err(err) => {
            error!("Error storing fault {} in the store: {}", fault.name, err);
            Err(ServerErrorResponse::new(err.code, err.message))
        }
    }
}

// get_fault is the handler of GET /fault/<fault_name> endpoint.
// On success, returns the fault config for the given fault name <fault_name> with 200 HTTP status.
// On failure, returns the error response with 500 HTTP status code.
#[get("/fault/<fault_name>", format = "json")]
pub fn get_fault(
    fault_name: String,
    fault_store: State<DB>,
) -> Result<Json<Fault>, ServerErrorResponse> {
    info!("Get fault by name: {:?}", fault_name);

    let fault_store = fault_store
        .read()
        .map_err(|err| ServerErrorResponse::new(LOCK_ERROR_CODE.to_string(), err.to_string()))?;

    match fault_store.get_by_fault_name(fault_name.as_str()) {
        Ok(fault) => {
            info!("Fault {} fetched from the store", fault_name);
            Ok(Json(fault))
        }
        Err(err) => {
            error!("Error fetching fault {}: {}", fault_name, err);
            Err(ServerErrorResponse::new(err.code, err.message))
        }
    }
}

// get_all_faults is the handler of GET /faults endpoint.
// On success, returns all the fault configs with 200 HTTP status code.
// On failure, returns the error response with 500 HTTP status code.
#[get("/faults", format = "json")]
pub fn get_all_faults(fault_store: State<DB>) -> Result<Json<Vec<Fault>>, ServerErrorResponse> {
    info!("Get all faults");
    let fault_store = fault_store
        .read()
        .map_err(|err| ServerErrorResponse::new(LOCK_ERROR_CODE.to_string(), err.to_string()))?;

    let faults = fault_store.get_all_faults();

    match faults {
        Err(err) => {
            error!("Error fetching all faults: {}", err);
            Err(ServerErrorResponse::new(err.code, err.message))
        }

        Ok(mut faults) => {
            faults.sort_by(|a, b| {
                b.last_modified
                    .unwrap()
                    .timestamp()
                    .cmp(&a.last_modified.unwrap().timestamp())
            });
            Ok(Json(faults))
        }
    }
}

// delete_fault is the handler of DELETE /fault/<fault_name> endpoint.
// DELETE /fault/<fault_name> endpoint is idempotent.
// On successful delete, it returns 204 No Content HTTP status.
// On failure, returns the error response with 500 HTTP status code.
#[delete("/fault/<fault_name>")]
pub fn delete_fault(
    fault_name: String,
    fault_store: State<DB>,
) -> Result<Status, ServerErrorResponse> {
    info!("Delete fault: {}", fault_name);

    let fault_store = fault_store
        .write()
        .map_err(|err| ServerErrorResponse::new(LOCK_ERROR_CODE.to_string(), err.to_string()))?;

    match fault_store.delete_fault(fault_name.as_str()) {
        Ok(_) => {
            debug!("Deleted fault: {:?}", fault_name);
            Ok(Status::NoContent)
        }
        Err(err) => {
            error!("Error deleting fault {}: {}", fault_name, err);
            Err(ServerErrorResponse::new(err.code, err.message))
        }
    }
}

// delete_all_faults is the handler for DELETE /faults.
// DELETE /faults endpoint is idempotent.
// On successful delete, it returns 204 No Content HTTP status.
// On failure, returns the error response with 500 HTTP status code.
#[delete("/faults")]
pub fn delete_all_faults(fault_store: State<DB>) -> Result<Status, ServerErrorResponse> {
    debug!("Delete all faults");

    let fault_store = fault_store
        .write()
        .map_err(|err| ServerErrorResponse::new(LOCK_ERROR_CODE.to_string(), err.to_string()))?;

    let faults = fault_store.get_all_faults().map_err(|err| {
        error!("Error fetching all faults: {}", err);
        ServerErrorResponse::new(err.code, err.message)
    })?;

    for fault in faults {
        match fault_store.delete_fault(fault.name.as_str()) {
            Ok(_) => {
                info!("Deleted fault: {}", fault.name);
            }
            Err(err) => {
                error!("Error deleting fault {}: {}", fault.name, err);
                return Err(ServerErrorResponse::new(err.code, err.message));
            }
        }
    }

    debug!("Deleted all faults from the store");
    Ok(Status::NoContent)
}

#[cfg(test)]
mod tests {
    use super::rocket;
    use super::*;
    use rocket::http::{ContentType, Status};
    use rocket::local::Client;
    use rocket::routes;

    fn setup_fault_config_server() -> rocket::Rocket {
        let fault_store = crate::store::mem_store::MemStore::new_db();
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
    }

    fn mock_store_util(client: &Client) {
        let response = client
            .post("/fault")
            .header(ContentType::JSON)
            .body(
                r#"{
                "name": "get_custom_err",
                "description": "GET custom error",
                "fault_type": "error",
                "error_msg": "KEY not found",
                "percentage": 100,
                "command": "GET"
            }
            "#,
            )
            .dispatch();

        assert_eq!(response.status(), Status::Created);
    }

    #[test]
    fn fault_config_server() {
        let client =
            Client::new(setup_fault_config_server()).expect("valid rocket client instance");
        mock_store_util(&client);
    }

    #[test]
    fn get_all_faults() {
        let client =
            Client::new(setup_fault_config_server()).expect("valid rocket client instance");
        mock_store_util(&client);

        let mut response = client.get("/faults").dispatch();
        assert_eq!(response.status(), Status::Ok);

        let response_string = response.body_string().unwrap();
        let faults: Vec<Fault> = serde_json::from_str(response_string.as_str()).unwrap();

        let expected_fault = Fault {
            name: "get_custom_err".to_string(),
            command: "GET".to_string(),
            description: Some("GET custom error".to_string()),
            fault_type: "error".to_string(),
            duration: None,
            error_msg: Some("KEY not found".to_string()),
            last_modified: faults[0].last_modified,
        };

        assert_eq!(faults.len(), 1);
        assert_eq!(faults[0], expected_fault);
    }

    #[test]
    fn get_fault() {
        let client =
            Client::new(setup_fault_config_server()).expect("valid rocket client instance");
        mock_store_util(&client);

        let mut response = client.get("/fault/get_custom_err").dispatch();
        assert_eq!(response.status(), Status::Ok);

        let response_string = response.body_string().unwrap();
        let fault: Fault = serde_json::from_str(response_string.as_str()).unwrap();

        let expected_fault = Fault {
            name: "get_custom_err".to_string(),
            command: "GET".to_string(),
            description: Some("GET custom error".to_string()),
            fault_type: "error".to_string(),
            duration: None,
            error_msg: Some("KEY not found".to_string()),
            last_modified: fault.last_modified,
        };

        assert_eq!(fault, expected_fault);
    }

    #[test]
    fn delete_fault() {
        let client =
            Client::new(setup_fault_config_server()).expect("valid rocket client instance");
        mock_store_util(&client);

        let delete_fault_response = client.delete("/fault/get_custom_err").dispatch();
        assert_eq!(delete_fault_response.status(), Status::NoContent);

        let mut get_faults_response = client.get("/faults").dispatch();
        assert_eq!(get_faults_response.status(), Status::Ok);

        let response_string = get_faults_response.body_string().unwrap();
        let faults: Vec<Fault> = serde_json::from_str(response_string.as_str()).unwrap();
        assert_eq!(faults.len(), 0);
    }

    #[test]
    fn delete_all_faults() {
        let client =
            Client::new(setup_fault_config_server()).expect("valid rocket client instance");
        mock_store_util(&client);

        let delete_all_faults_response = client.delete("/faults").dispatch();
        assert_eq!(delete_all_faults_response.status(), Status::NoContent);

        let mut get_faults_response = client.get("/faults").dispatch();
        assert_eq!(get_faults_response.status(), Status::Ok);

        let response_string = get_faults_response.body_string().unwrap();
        let faults: Vec<Fault> = serde_json::from_str(response_string.as_str()).unwrap();
        assert_eq!(faults.len(), 0);
    }
}

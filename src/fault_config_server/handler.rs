use crate::store::fault_store::{Fault, DB};
use chrono::Utc;
use log::{debug, error, info};

use actix_web::{http::StatusCode, HttpResponseBuilder, ResponseError};
use actix_web::{web, HttpRequest, HttpResponse};

// store_fault is the handler of POST /fault endpoint.
// On success, returns the response with Created (201) HTTP status.
// On failure, returns the response with Internal Server Error (500) HTTP status.
pub async fn store_fault(
    fault: web::Json<Fault>,
    fault_store: web::Data<DB>,
) -> Result<HttpResponseBuilder, ServerErrorResponse> {
    info!("Create fault: fault name: {:?}", fault.name);
    let mut fault = fault.clone();
    fault.last_modified = Some(Utc::now());

    let faults = fault_store
        .read()
        .await
        .get_all_faults()
        .map_err(|err| ServerErrorResponse {
            status_code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
            message: err.message,
        })?;

    for f in faults {
        if f.command == fault.command {
            return Err(ServerErrorResponse::new(
                StatusCode::CONFLICT,
                format!(
                    "There already exists a fault for the same {} command",
                    fault.command
                ),
            ));
        }
    }

    match fault_store.write().await.store(&fault.name, &fault) {
        Ok(_) => {
            info!("Fault {} created in the store", fault.name);
            Ok(HttpResponse::Created())
        }

        Err(err) => {
            error!("Error storing fault {} in the store: {}", fault.name, err);
            Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.message,
            ))
        }
    }
}

// get_fault is the handler of GET /fault/<fault_name> endpoint.
// On success, returns the fault config for the given fault name <fault_name> with 200 HTTP status.
// On failure, returns the error response with 500 HTTP status code.
pub async fn get_fault(
    request: HttpRequest,
    fault_store: web::Data<DB>,
) -> Result<HttpResponse, ServerErrorResponse> {
    let fault_name = request.match_info().get("fault_name").ok_or_else(|| {
        ServerErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error fetching fault name from the request path".to_string(),
        )
    })?;
    info!("Fetch fault by name: {:?}", fault_name);

    match fault_store.read().await.get_by_fault_name(fault_name) {
        Ok(fault) => {
            info!("Fault {} fetched from the store", fault_name);
            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(fault))
        }
        Err(err) => {
            error!("Error fetching fault {}: {}", fault_name, err);
            Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ))
        }
    }
}

// get_all_faults is the handler of GET /faults endpoint.
// On success, returns all the fault configs with 200 HTTP status code.
// On failure, returns the error response with 500 HTTP status code.
pub async fn get_all_faults(
    fault_store: web::Data<DB>,
) -> Result<HttpResponse, ServerErrorResponse> {
    info!("Fetch all faults");
    let faults = fault_store.read().await.get_all_faults();

    match faults {
        Ok(mut faults) => {
            faults.sort_by(|a, b| {
                b.last_modified
                    .unwrap()
                    .timestamp()
                    .cmp(&a.last_modified.unwrap().timestamp())
            });

            Ok(HttpResponse::Ok()
                .content_type("application/json")
                .json(faults))
        }
        Err(err) => {
            error!("Error fetching all faults: {}", err);
            Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.message,
            ))
        }
    }
}

// delete_fault is the handler of DELETE /fault/<fault_name> endpoint.
// DELETE /fault/<fault_name> endpoint is idempotent.
// On successful delete, it returns 204 No Content HTTP status.
// On failure, returns the error response with 500 HTTP status code.
pub async fn delete_fault(
    request: HttpRequest,
    fault_store: web::Data<DB>,
) -> Result<HttpResponseBuilder, ServerErrorResponse> {
    let fault_name = request.match_info().get("fault_name").ok_or_else(|| {
        ServerErrorResponse::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Error fetching fault name from the request path".to_string(),
        )
    })?;
    info!("Delete fault: {}", fault_name);

    match fault_store.write().await.delete_fault(fault_name) {
        Ok(_) => {
            debug!("Deleted fault: {:?}", fault_name);
            Ok(HttpResponse::NoContent())
        }
        Err(err) => {
            error!("Error deleting fault {}: {}", fault_name, err);
            Err(ServerErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.message,
            ))
        }
    }
}

// delete_all_faults is the handler for DELETE /faults.
// DELETE /faults endpoint is idempotent.
// On successful delete, it returns 204 No Content HTTP status.
// On failure, returns the error response with 500 HTTP status code.
pub async fn delete_all_faults(
    fault_store: web::Data<DB>,
) -> Result<HttpResponseBuilder, ServerErrorResponse> {
    debug!("Delete all faults");

    let fault_store = fault_store.write().await;
    let faults = fault_store.get_all_faults().map_err(|err| {
        error!("Error fetching all faults: {}", err);
        ServerErrorResponse::new(StatusCode::INTERNAL_SERVER_ERROR, err.message)
    })?;

    for fault in faults {
        match fault_store.delete_fault(fault.name.as_str()) {
            Ok(_) => {
                info!("Deleted fault: {}", fault.name);
            }
            Err(err) => {
                error!("Error deleting fault {}: {}", fault.name, err);
                return Err(ServerErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    err.message,
                ));
            }
        }
    }

    debug!("Deleted all faults from the store");
    Ok(HttpResponse::NoContent())
}

#[derive(serde::Serialize)]
pub struct ServerErrorResponse {
    status_code: u16,
    message: String,
}

impl ServerErrorResponse {
    fn new(status_code: StatusCode, message: String) -> Self {
        Self {
            status_code: status_code.as_u16(),
            message,
        }
    }
}

impl std::fmt::Debug for ServerErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error: {}", self.message)
    }
}

impl std::fmt::Display for ServerErrorResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "error: {}", self.message)
    }
}

impl ResponseError for ServerErrorResponse {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    fn error_response(&self) -> HttpResponse {
        HttpResponseBuilder::new(StatusCode::from_u16(self.status_code).unwrap())
            .content_type("application/json")
            .json(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{http::StatusCode, test, web, web::Data, App};

    #[tokio::test]
    async fn test_store_fault() {
        let fault_store = crate::store::mem_store::MemStore::new_db();
        let mut app = test::init_service(
            App::new()
                .route("/fault", web::post().to(store_fault))
                .app_data(Data::new(fault_store.clone())),
        )
        .await;

        let req = test::TestRequest::post()
            .uri("/fault")
            .set_json(get_mock_fault())
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_conflict_store() {
        let fault_store = crate::store::mem_store::MemStore::new_db();
        let mut app = test::init_service(
            App::new()
                .route("/fault", web::post().to(store_fault))
                .app_data(Data::new(fault_store.clone())),
        )
        .await;

        let mut req = test::TestRequest::post()
            .uri("/fault")
            .set_json(get_mock_fault())
            .to_request();
        let mut resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::CREATED);

        req = test::TestRequest::post()
            .uri("/fault")
            .set_json(get_mock_fault())
            .to_request();
        resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_get_all_faults() {
        let fault_store = crate::store::mem_store::MemStore::new_db();
        let fault = get_mock_fault();
        fault_store
            .write()
            .await
            .store(&fault.name, &fault)
            .unwrap();

        let mut app = test::init_service(
            App::new()
                .route("/faults", web::get().to(get_all_faults))
                .app_data(Data::new(fault_store)),
        )
        .await;

        let req = test::TestRequest::get().uri("/faults").to_request();
        let resp = test::call_service(&mut app, req).await;
        let result = test::read_body(resp).await;
        let faults: Vec<Fault> = serde_json::from_slice(&result).unwrap();

        assert_eq!(faults.len(), 1);
        assert_eq!(faults[0], get_mock_fault());
    }

    #[tokio::test]
    async fn test_get_fault() {
        let fault_store = crate::store::mem_store::MemStore::new_db();
        let fault = get_mock_fault();
        fault_store
            .write()
            .await
            .store(&fault.name, &fault)
            .unwrap();

        let mut app = test::init_service(
            App::new()
                .route("/fault/{fault_name}", web::get().to(get_fault))
                .app_data(Data::new(fault_store)),
        )
        .await;

        let req = test::TestRequest::get()
            .uri(format!("/fault/{}", fault.name).as_str())
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        let result = test::read_body(resp).await;

        let fault: Fault = serde_json::from_slice(&result).unwrap();
        assert_eq!(fault, get_mock_fault());
    }

    #[tokio::test]
    async fn test_delete_fault() {
        let fault_store = crate::store::mem_store::MemStore::new_db();
        let fault = get_mock_fault();
        fault_store
            .write()
            .await
            .store(&fault.name, &fault)
            .unwrap();

        let mut app = test::init_service(
            App::new()
                .route("/fault/{fault_name}", web::delete().to(delete_fault))
                .app_data(Data::new(fault_store)),
        )
        .await;

        let req = test::TestRequest::delete()
            .uri(format!("/fault/{}", fault.name).as_str())
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn test_delete_all_faults() {
        let fault_store = crate::store::mem_store::MemStore::new_db();
        let fault = get_mock_fault();
        fault_store
            .write()
            .await
            .store(&fault.name, &fault)
            .unwrap();

        let mut app = test::init_service(
            App::new()
                .route("/faults", web::delete().to(delete_all_faults))
                .app_data(Data::new(fault_store)),
        )
        .await;

        let req = test::TestRequest::delete().uri("/faults").to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);
    }

    fn get_mock_fault() -> Fault {
        Fault {
            name: "get_custom_err".to_string(),
            description: Some("GET custom error".to_string()),
            fault_type: "error".to_string(),
            error_msg: Some("KEY not found".to_string()),
            duration: None,
            command: "GET".to_string(),
            last_modified: None,
        }
    }
}

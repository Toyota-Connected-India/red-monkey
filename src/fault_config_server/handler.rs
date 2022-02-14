use crate::store::fault_store::{Fault, FaultVariants, DB};
use chrono::Utc;
use std::string::ToString;
use tracing::{debug, error, info};

use actix_web::{
    http::{header::ContentType, StatusCode},
    HttpResponseBuilder, ResponseError,
};
use actix_web::{web, HttpRequest, HttpResponse};

/// store_fault is the handler of POST /fault endpoint.
///
/// 1. When the fault is successfully stored in the fault store, HTTP Created 201 is retuned.
/// 2. For invalid POST body payload, HTTP Bad request 400 is returned.
/// 3. When the fault that is posted conflicts with the current state of the fault store, HTTP
///    Conflict 409 is returned.
/// 4. If the fault type is not one of [`delay`, `error`] value, HTTP Bad request would be returned.
/// 5. When the fault fails to be stored in the fault store, HTTP Internal Server Error 500 is returned.
#[tracing::instrument(skip(fault_store))]
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

    if fault.fault_type != FaultVariants::Delay.as_str()
        && fault.fault_type != FaultVariants::Error.as_str()
    {
        return Err(ServerErrorResponse {
            status_code: StatusCode::BAD_REQUEST.as_u16(),
            message: format!("Error as unsupported fault type: {}", fault.fault_type),
        });
    }

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

/// get_fault is the handler of GET /fault/<fault_name> endpoint.
///
/// 1. On successful fetch, returns the fault configuration of the given fault <fault_name> with
///    HTTP status OK.
/// 2. If the given fault name is not available in the fault store, HTTP Bad request 400 is
///    returned.
#[tracing::instrument(skip(fault_store, request))]
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
                .content_type(ContentType::json())
                .json(fault))
        }
        Err(err) => {
            error!("Error fetching fault {}: {}", fault_name, err);
            Err(ServerErrorResponse::new(
                StatusCode::BAD_REQUEST,
                err.to_string(),
            ))
        }
    }
}

/// get_all_faults is the handler of GET /faults endpoint.
///
/// 1. On success fetch, returns all the fault configurations with HTTP status 200.
/// 2. If unable to fetch the fault configurations from the fault store, HTTP Internal Server Error
///    is returned.
#[tracing::instrument(skip(fault_store))]
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
                .content_type(ContentType::json())
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

/// delete_fault is the handler of DELETE /fault/<fault_name> endpoint.
///
/// 1. DELETE /fault/<fault_name> endpoint is idempotent.
/// 2. On successful delete, HTTP No Content 204 status is returned.
/// 3. On failing to delete the given fault <fault_name>, HTTP Internal Server Error 500 is returned.
#[tracing::instrument(skip(fault_store, request))]
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

/// delete_all_faults is the handler for DELETE /faults.
///
/// DELETE /faults endpoint is idempotent.
/// On successful delete, it returns 204 No Content HTTP status.
/// On failing to delete all faults, returns HTTP Internal Server Error 500 status.
#[tracing::instrument(skip(fault_store))]
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
            .content_type(ContentType::json())
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
    async fn test_store_fault_against_invalid_fault_type() {
        let fault_store = crate::store::mem_store::MemStore::new_db();
        let mut app = test::init_service(
            App::new()
                .route("/fault", web::post().to(store_fault))
                .app_data(Data::new(fault_store.clone())),
        )
        .await;

        let mut fault = get_mock_fault();
        fault.fault_type = "invalid_fault_type".to_string();

        let req = test::TestRequest::post()
            .uri("/fault")
            .set_json(fault)
            .to_request();
        let resp = test::call_service(&mut app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
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

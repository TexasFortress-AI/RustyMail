use actix_web::test;
use imap_api_rust::{
    api::routes::configure_routes,
    models::{
        email::{EmailBody, EmailCreateRequest, EmailMoveRequest},
        folder::FolderCreateRequest,
    },
};
use std::sync::Mutex;
use native_tls::TlsStream;
use std::net::TcpStream;
use imap as imap_crate;

type ActualImapSession = imap_crate::Session<TlsStream<TcpStream>>;

#[actix_web::test]
async fn test_homepage() {
    let app = test::init_service(
        actix_web::App::new()
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    ).await;
    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_api_docs() {
    let app = test::init_service(
        actix_web::App::new()
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    ).await;
    let req = test::TestRequest::get().uri("/api-docs").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_list_emails() {
    let app = test::init_service(
        actix_web::App::new()
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    ).await;
    let req = test::TestRequest::get()
        .uri("/emails/INBOX")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_list_unread_emails() {
    let app = test::init_service(
        actix_web::App::new()
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    ).await;
    let req = test::TestRequest::get()
        .uri("/emails/INBOX/unread")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_get_single_email() {
    let app = test::init_service(
        actix_web::App::new()
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    ).await;
    let req = test::TestRequest::get()
        .uri("/emails/INBOX/1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_move_email() {
    let app = test::init_service(
        actix_web::App::new()
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    ).await;
    let move_request = EmailMoveRequest {
        uid: "1".to_string(),
        source_folder: "INBOX".to_string(),
        dest_folder: "Trash".to_string(),
    };
    let req = test::TestRequest::post()
        .uri("/emails/move")
        .set_json(&move_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_create_email() {
    let app = test::init_service(
        actix_web::App::new()
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    ).await;
    let create_request = EmailCreateRequest {
        subject: "Test Subject".to_string(),
        body: EmailBody {
            text_plain: Some("Test content".to_string()),
            text_html: None,
        },
        to: vec!["test@example.com".to_string()],
        cc: None,
        bcc: None,
        attachments: None,
    };
    let req = test::TestRequest::post()
        .uri("/emails/INBOX")
        .set_json(&create_request)
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

#[actix_web::test]
async fn test_delete_email() {
    let app = test::init_service(
        actix_web::App::new()
            .configure(configure_routes::<Mutex<ActualImapSession>>)
    ).await;
    let req = test::TestRequest::delete()
        .uri("/emails/INBOX/1")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
} 
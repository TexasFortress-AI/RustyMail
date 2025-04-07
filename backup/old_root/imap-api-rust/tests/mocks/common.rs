use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse},
    web, App, Error,
};
use imap_api_rust::{
    api::routes::configure_routes,
    imap::client::{ImapClient, ZeroCopy},
};
use std::sync::Arc;
use super::mock::{MockImapSession, MockImapSessionWrapper};
use tera::Tera;

// Setup test application data with a working mock IMAP client
pub async fn setup_test_app_data() -> web::Data<ImapClient<MockImapSessionWrapper>> {
    let mut mock = MockImapSession::new();
    mock.expect_list()
        .returning(|| Ok(vec!["INBOX".to_string()]));
    mock.expect_create()
        .returning(|_| Ok(()));
    mock.expect_delete()
        .returning(|_| Ok(()));
    mock.expect_select()
        .returning(|_| Ok(()));
    mock.expect_search()
        .returning(|_| Ok(vec![1, 2, 3]));
    mock.expect_fetch()
        .returning(|_| Ok(vec!["Email content".to_string()]));
    mock.expect_uid_fetch()
        .returning(|_| Ok(vec!["Email content".to_string()]));
    mock.expect_uid_move()
        .returning(|_, _| Ok(()));
    mock.expect_rename()
        .returning(|_, _| Ok(()));
    mock.expect_logout()
        .returning(|| Ok(()));

    let session = MockImapSessionWrapper { inner: mock };
    let client = ImapClient::new(session);
    web::Data::new(client)
}

// Setup test application data with a failing mock IMAP client
pub fn setup_failing_test_app_data() -> web::Data<Arc<Mutex<ImapClient<MockImapSessionWrapper>>>> {
    let mock = MockImapSession::new(true);
    let wrapper = MockImapSessionWrapper::new(mock);
    let client = ImapClient::new(wrapper);
    web::Data::new(Arc::new(Mutex::new(client)))
}

pub async fn setup_mock_test_app() -> impl Service<ServiceRequest, Response = ServiceResponse, Error = Error> {
    let app_data = setup_test_app_data().await;
    App::new()
        .app_data(app_data.clone())
        .configure(configure_routes::<MockImapSessionWrapper>)
        .service(web::scope(""))
}

pub fn setup_mock_test_app_with_tera() -> web::Data<Arc<Mutex<ImapClient<MockImapSessionWrapper>>>> {
    let mock = MockImapSession::new(false);
    let wrapper = MockImapSessionWrapper::new(mock);
    let client = ImapClient::new(wrapper);
    let tera = Tera::new("templates/**/*").unwrap_or_else(|_| Tera::default());
    web::Data::new(Arc::new(Mutex::new(client)))
} 
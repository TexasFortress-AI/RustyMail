use actix_web::web;
use crate::{
    api::routes::configure_routes,
    imap::client::ImapClient,
};
use super::mock::MockImapSession;
use std::sync::{Arc, Mutex};

// Setup test application data with a working mock IMAP client
pub fn setup_test_app_data() -> web::Data<Arc<Mutex<ImapClient<MockImapSession>>>> {
    let mock = MockImapSession::new();
    let client = ImapClient::new(Arc::new(mock));
    web::Data::new(Arc::new(Mutex::new(client)))
}

// Setup test application data with a failing mock IMAP client
pub fn setup_failing_test_app_data() -> web::Data<Arc<Mutex<ImapClient<MockImapSession>>>> {
    let mut mock = MockImapSession::new();
    mock.should_fail = true;
    let client = ImapClient::new(Arc::new(mock));
    web::Data::new(Arc::new(Mutex::new(client)))
} 
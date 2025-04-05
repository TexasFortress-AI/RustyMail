use criterion::{criterion_group, criterion_main, Criterion};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use imap_api_rust::api::routes::configure_routes;
use imap_api_rust::imap::client::ImapClient;
use imap_api_rust::models::config::ImapConfig;
use actix_web::{test, web, App};
use tera::Tera;
use native_tls::{TlsConnector, TlsStream};
use imap as imap_crate;
use std::net::TcpStream;

// Constants for benchmark
const IMAP_SERVER: &str = "p3plzcpnl505455.prod.phx3.secureserver.net";
const IMAP_PORT: u16 = 993;
const IMAP_USERNAME: &str = "info@texasfortress.ai";
const IMAP_PASSWORD: &str = "M0P@fc9#fy2Kr1TC";

// Define the concrete session type
type ActualImapSession = imap_crate::Session<TlsStream<TcpStream>>;

// Get a real IMAP client for testing - matching the expected structure in handlers
fn get_imap_client_for_test() -> Arc<Mutex<ImapClient<Mutex<ActualImapSession>>>> {
    // Set up TLS
    let tls = TlsConnector::builder()
        .build()
        .expect("Failed to build TLS connector");

    // Connect to IMAP server
    let client = imap_crate::connect((IMAP_SERVER, IMAP_PORT), IMAP_SERVER, &tls)
        .expect("Failed to connect to IMAP server");

    // Login to IMAP server
    let imap_session = client
        .login(IMAP_USERNAME, IMAP_PASSWORD)
        .expect("Failed to login to IMAP server");

    // Wrap the actual session in Mutex<ActualImapSession>
    let session_mutex = Mutex::new(imap_session);

    // Create the ImapClient with the session
    let imap_client = ImapClient::new(Arc::new(session_mutex));

    // Wrap the ImapClient in a Mutex to match the handler's expected type
    Arc::new(Mutex::new(imap_client))
}

// Helper to initialize the Actix App for testing with a real IMAP client
fn setup_test_app() -> App<
    impl actix_web::dev::ServiceFactory<
        actix_web::dev::ServiceRequest,
        Config = (),
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let imap_client = get_imap_client_for_test();
    // Minimal Tera setup for tests
    let tera = Tera::new("templates/**/*").unwrap_or_else(|_| Tera::default());

    App::new()
        .app_data(web::Data::new(imap_client.clone()))
        .app_data(web::Data::new(tera.clone()))
        .configure(configure_routes::<Mutex<ActualImapSession>>)
}

// Benchmark folder listing endpoint
async fn bench_list_folders() {
    let app = test::init_service(setup_test_app()).await;
    let req = test::TestRequest::get()
        .uri("/folders")
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

// Benchmark get folder stats endpoint
async fn bench_folder_stats() {
    let app = test::init_service(setup_test_app()).await;
    let req = test::TestRequest::get()
        .uri("/folders/INBOX/stats")
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

// Benchmark list emails in folder endpoint
async fn bench_list_emails() {
    let app = test::init_service(setup_test_app()).await;
    let req = test::TestRequest::get()
        .uri("/emails/INBOX")
        .to_request();
    
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}

// Setup the benchmark
pub fn api_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    
    // Create a group for folder operations with appropriate settings
    let mut folder_group = c.benchmark_group("folder_operations");
    folder_group.sample_size(10); // Reduce sample size for external API calls
    folder_group.measurement_time(Duration::from_secs(15));
    
    // Benchmark list folders
    folder_group.bench_function("list_folders", |b| {
        b.iter(|| {
            rt.block_on(bench_list_folders())
        });
    });
    
    // Benchmark folder stats
    folder_group.bench_function("folder_stats", |b| {
        b.iter(|| {
            rt.block_on(bench_folder_stats())
        });
    });
    
    folder_group.finish();
    
    // Create a group for email operations
    let mut email_group = c.benchmark_group("email_operations");
    email_group.sample_size(10); // Reduce sample size for external API calls
    email_group.measurement_time(Duration::from_secs(15));
    
    // Benchmark list emails
    email_group.bench_function("list_emails", |b| {
        b.iter(|| {
            rt.block_on(bench_list_emails())
        });
    });
    
    email_group.finish();
}

criterion_group!(benches, api_benchmark);
criterion_main!(benches); 
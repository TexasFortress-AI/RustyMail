use actix_web::web;
use crate::api::handlers::{
    list_folders_handler, create_folder_handler, delete_folder_handler,
    rename_folder_handler, get_folder_stats_handler, list_emails_handler,
    list_unread_emails_handler, get_email_handler, move_email_handler
};
use crate::imap::client::ImapSessionTrait;

pub fn configure_routes<S: ImapSessionTrait + 'static>(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/folders")
            .route("", web::get().to(list_folders_handler::<S>))
            .route("", web::post().to(create_folder_handler::<S>))
            // Note: Path parameters need to be defined in the route
            .route("/{folder}", web::delete().to(delete_folder_handler::<S>))
            .route("/{folder}/rename", web::put().to(rename_folder_handler::<S>))
            .route("/{folder}/stats", web::get().to(get_folder_stats_handler::<S>))
    );

    // Scope for /emails routes
    cfg.service(
        web::scope("/emails")
            .route("/{folder}", web::get().to(list_emails_handler::<S>))
            .route("/{folder}/unread", web::get().to(list_unread_emails_handler::<S>))
            .route("/{folder}/{uid}", web::get().to(get_email_handler::<S>))
            .route("/move", web::post().to(move_email_handler::<S>))
    );

    // TODO: Add routes for / and /api-docs
}

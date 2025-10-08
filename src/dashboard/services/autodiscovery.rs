use hickory_resolver::TokioResolver;
use log::{info, debug, warn, error};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use reqwest::Client;

#[derive(Error, Debug)]
pub enum AutodiscoveryError {
    #[error("DNS lookup failed: {0}")]
    DnsError(String),
    #[error("HTTP request failed: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("XML parsing failed: {0}")]
    XmlError(String),
    #[error("No configuration found for domain: {0}")]
    NoConfigFound(String),
    #[error("Invalid email address: {0}")]
    InvalidEmail(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailConfig {
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_use_tls: bool,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_use_tls: Option<bool>,
    pub smtp_use_starttls: Option<bool>,
    pub username_pattern: String, // e.g., "%EMAILADDRESS%" or "%EMAILLOCALPART%"
}

pub struct AutodiscoveryService {
    resolver: TokioResolver,
    http_client: Client,
}

impl AutodiscoveryService {
    pub fn new() -> Result<Self, AutodiscoveryError> {
        // Use default system resolver configuration with Tokio runtime
        let resolver = TokioResolver::builder_tokio()
            .map_err(|e| AutodiscoveryError::DnsError(e.to_string()))?
            .build();

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| AutodiscoveryError::HttpError(e))?;

        Ok(Self {
            resolver,
            http_client,
        })
    }

    /// Discover email configuration for a given email address
    pub async fn discover(&self, email: &str) -> Result<EmailConfig, AutodiscoveryError> {
        info!("Starting autodiscovery for email: {}", email);

        let domain = self.extract_domain(email)?;
        debug!("Extracted domain: {}", domain);

        // Try RFC 6186 DNS SRV records first
        if let Ok(config) = self.try_rfc6186(&domain).await {
            info!("Found configuration via RFC 6186 for {}", domain);
            return Ok(config);
        }

        // Try Mozilla Autoconfig
        if let Ok(config) = self.try_mozilla_autoconfig(email, &domain).await {
            info!("Found configuration via Mozilla Autoconfig for {}", domain);
            return Ok(config);
        }

        Err(AutodiscoveryError::NoConfigFound(domain))
    }

    fn extract_domain(&self, email: &str) -> Result<String, AutodiscoveryError> {
        email.split('@')
            .nth(1)
            .map(|s| s.to_string())
            .ok_or_else(|| AutodiscoveryError::InvalidEmail(email.to_string()))
    }

    /// Try RFC 6186 DNS SRV record lookup
    async fn try_rfc6186(&self, domain: &str) -> Result<EmailConfig, AutodiscoveryError> {
        debug!("Attempting RFC 6186 autodiscovery for domain: {}", domain);

        // Try to find IMAPS SRV record first (more secure)
        let imap_record = format!("_imaps._tcp.{}", domain);
        let smtp_record = format!("_submission._tcp.{}", domain);

        let imap_result = self.resolver.srv_lookup(&imap_record).await;

        match imap_result {
            Ok(srv_records) => {
                if let Some(srv) = srv_records.iter().next() {
                    debug!("Found IMAPS SRV record: target={}, port={}", srv.target(), srv.port());

                    let mut config = EmailConfig {
                        imap_host: srv.target().to_string().trim_end_matches('.').to_string(),
                        imap_port: srv.port(),
                        imap_use_tls: true, // IMAPS uses implicit TLS
                        smtp_host: None,
                        smtp_port: None,
                        smtp_use_tls: None,
                        smtp_use_starttls: None,
                        username_pattern: "%EMAILADDRESS%".to_string(),
                    };

                    // Try to find SMTP submission service
                    if let Ok(smtp_records) = self.resolver.srv_lookup(&smtp_record).await {
                        if let Some(smtp_srv) = smtp_records.iter().next() {
                            debug!("Found SMTP submission SRV record: target={}, port={}", smtp_srv.target(), smtp_srv.port());
                            config.smtp_host = Some(smtp_srv.target().to_string().trim_end_matches('.').to_string());
                            config.smtp_port = Some(smtp_srv.port());
                            config.smtp_use_tls = Some(smtp_srv.port() == 465); // Port 465 uses implicit TLS
                            config.smtp_use_starttls = Some(smtp_srv.port() == 587); // Port 587 uses STARTTLS
                        }
                    }

                    return Ok(config);
                }
            }
            Err(e) => {
                debug!("IMAPS SRV lookup failed, trying IMAP with STARTTLS: {}", e);

                // Fallback: try IMAP with STARTTLS
                let imap_starttls_record = format!("_imap._tcp.{}", domain);
                if let Ok(srv_records) = self.resolver.srv_lookup(&imap_starttls_record).await {
                    if let Some(srv) = srv_records.iter().next() {
                        debug!("Found IMAP SRV record: target={}, port={}", srv.target(), srv.port());

                        let mut config = EmailConfig {
                            imap_host: srv.target().to_string().trim_end_matches('.').to_string(),
                            imap_port: srv.port(),
                            imap_use_tls: srv.port() == 993, // Port 993 = IMAPS, otherwise STARTTLS
                            smtp_host: None,
                            smtp_port: None,
                            smtp_use_tls: None,
                            smtp_use_starttls: None,
                            username_pattern: "%EMAILADDRESS%".to_string(),
                        };

                        // Try to find SMTP submission service
                        if let Ok(smtp_records) = self.resolver.srv_lookup(&smtp_record).await {
                            if let Some(smtp_srv) = smtp_records.iter().next() {
                                config.smtp_host = Some(smtp_srv.target().to_string().trim_end_matches('.').to_string());
                                config.smtp_port = Some(smtp_srv.port());
                                config.smtp_use_tls = Some(smtp_srv.port() == 465);
                                config.smtp_use_starttls = Some(smtp_srv.port() == 587);
                            }
                        }

                        return Ok(config);
                    }
                }
            }
        }

        Err(AutodiscoveryError::DnsError(format!("No SRV records found for {}", domain)))
    }

    /// Try Mozilla Autoconfig protocol
    async fn try_mozilla_autoconfig(&self, email: &str, domain: &str) -> Result<EmailConfig, AutodiscoveryError> {
        debug!("Attempting Mozilla Autoconfig for domain: {}", domain);

        // Try both autoconfig URLs as per Mozilla spec
        let urls = vec![
            format!("https://autoconfig.{}/mail/config-v1.1.xml?emailaddress={}", domain, email),
            format!("https://{}/.well-known/autoconfig/mail/config-v1.1.xml?emailaddress={}", domain, email),
            // Fallback to HTTP if HTTPS fails (less secure but some providers only support HTTP)
            format!("http://autoconfig.{}/mail/config-v1.1.xml?emailaddress={}", domain, email),
        ];

        for url in urls {
            debug!("Trying Mozilla Autoconfig URL: {}", url);

            match self.http_client.get(&url).send().await {
                Ok(response) if response.status().is_success() => {
                    let xml_text = response.text().await?;
                    debug!("Received autoconfig XML response");

                    match self.parse_mozilla_autoconfig(&xml_text) {
                        Ok(config) => return Ok(config),
                        Err(e) => {
                            warn!("Failed to parse autoconfig XML from {}: {}", url, e);
                            continue;
                        }
                    }
                }
                Ok(response) => {
                    debug!("Autoconfig URL {} returned status: {}", url, response.status());
                }
                Err(e) => {
                    debug!("Failed to fetch autoconfig from {}: {}", url, e);
                }
            }
        }

        Err(AutodiscoveryError::NoConfigFound(domain.to_string()))
    }

    /// Parse Mozilla Autoconfig XML
    fn parse_mozilla_autoconfig(&self, xml: &str) -> Result<EmailConfig, AutodiscoveryError> {
        use quick_xml::de::from_str;
        use serde::Deserialize;

        #[derive(Debug, Deserialize)]
        struct ClientConfig {
            #[serde(rename = "emailProvider")]
            email_provider: EmailProvider,
        }

        #[derive(Debug, Deserialize)]
        struct EmailProvider {
            #[serde(rename = "incomingServer", default)]
            incoming_servers: Vec<IncomingServer>,
            #[serde(rename = "outgoingServer", default)]
            outgoing_servers: Vec<OutgoingServer>,
        }

        #[derive(Debug, Deserialize)]
        struct IncomingServer {
            #[serde(rename = "type")]
            server_type: String,
            hostname: String,
            port: u16,
            #[serde(rename = "socketType")]
            socket_type: String,
            username: String,
        }

        #[derive(Debug, Deserialize)]
        struct OutgoingServer {
            hostname: String,
            port: u16,
            #[serde(rename = "socketType")]
            socket_type: String,
            username: String,
        }

        let config: ClientConfig = from_str(xml)
            .map_err(|e| AutodiscoveryError::XmlError(e.to_string()))?;

        // Find IMAP server (prefer IMAP over POP3)
        let imap_server = config.email_provider.incoming_servers
            .iter()
            .find(|s| s.server_type.to_lowercase() == "imap")
            .ok_or_else(|| AutodiscoveryError::XmlError("No IMAP server found".to_string()))?;

        // Parse socket type for TLS settings
        let imap_use_tls = imap_server.socket_type.to_uppercase() == "SSL";

        let mut email_config = EmailConfig {
            imap_host: imap_server.hostname.clone(),
            imap_port: imap_server.port,
            imap_use_tls,
            smtp_host: None,
            smtp_port: None,
            smtp_use_tls: None,
            smtp_use_starttls: None,
            username_pattern: imap_server.username.clone(),
        };

        // Add SMTP config if available
        if let Some(smtp_server) = config.email_provider.outgoing_servers.first() {
            let smtp_use_tls = smtp_server.socket_type.to_uppercase() == "SSL";
            let smtp_use_starttls = smtp_server.socket_type.to_uppercase() == "STARTTLS";

            email_config.smtp_host = Some(smtp_server.hostname.clone());
            email_config.smtp_port = Some(smtp_server.port);
            email_config.smtp_use_tls = Some(smtp_use_tls);
            email_config.smtp_use_starttls = Some(smtp_use_starttls);
        }

        Ok(email_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_domain() {
        let service = AutodiscoveryService::new().unwrap();
        assert_eq!(service.extract_domain("user@example.com").unwrap(), "example.com");
        assert_eq!(service.extract_domain("test@gmail.com").unwrap(), "gmail.com");
        assert!(service.extract_domain("invalid-email").is_err());
    }

    #[tokio::test]
    async fn test_parse_mozilla_autoconfig() {
        let service = AutodiscoveryService::new().unwrap();

        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<clientConfig version="1.1">
  <emailProvider id="example.com">
    <incomingServer type="imap">
      <hostname>imap.example.com</hostname>
      <port>993</port>
      <socketType>SSL</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </incomingServer>
    <outgoingServer type="smtp">
      <hostname>smtp.example.com</hostname>
      <port>587</port>
      <socketType>STARTTLS</socketType>
      <username>%EMAILADDRESS%</username>
      <authentication>password-cleartext</authentication>
    </outgoingServer>
  </emailProvider>
</clientConfig>"#;

        let config = service.parse_mozilla_autoconfig(xml).unwrap();
        assert_eq!(config.imap_host, "imap.example.com");
        assert_eq!(config.imap_port, 993);
        assert_eq!(config.imap_use_tls, true);
        assert_eq!(config.smtp_host.unwrap(), "smtp.example.com");
        assert_eq!(config.smtp_port.unwrap(), 587);
        assert_eq!(config.smtp_use_starttls.unwrap(), true);
    }
}

# Task ID: 91

**Title:** Create Evidence Export Tool for Attorney Review

**Status:** pending

**Dependencies:** 84, 90

**Priority:** medium

**Description:** Build MCP tool to package emails and attachments into organized evidence folders with manifest files

**Details:**

Implement export_evidence MCP tool:

```rust
// In mcp_tools/export_evidence.rs
use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use csv::Writer;

#[derive(Debug, Serialize)]
pub struct ExportRequest {
    pub account_id: i64,
    pub folder: Option<String>,
    pub search_query: Option<String>,
    pub output_path: String,
    pub date_range: Option<DateRange>,
}

#[derive(Debug, Serialize)]
pub struct ManifestEntry {
    pub email_date: String,
    pub from: String,
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub synopsis: String,
    pub attachment_paths: Vec<String>,
}

pub async fn export_evidence(request: ExportRequest) -> Result<String> {
    // Create output directory structure
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let export_dir = PathBuf::from(&request.output_path)
        .join(format!("evidence_export_{}_{}", request.account_id, timestamp));
    
    let emails_dir = export_dir.join("emails");
    let attachments_dir = export_dir.join("attachments");
    
    fs::create_dir_all(&emails_dir)?;
    fs::create_dir_all(&attachments_dir)?;
    
    // Get emails based on criteria
    let emails = if let Some(query) = request.search_query {
        search_emails(request.account_id, &query).await?
    } else if let Some(folder) = request.folder {
        get_folder_emails(request.account_id, &folder).await?
    } else {
        return Err(anyhow!("Must specify either folder or search_query"));
    };
    
    // Create manifest CSV
    let manifest_path = export_dir.join("manifest.csv");
    let mut csv_writer = Writer::from_path(&manifest_path)?;
    csv_writer.write_record(&["Date", "From", "To", "CC", "Subject", "Synopsis", "Attachments"])?;
    
    // Also create markdown summary
    let mut markdown = String::from("# Email Evidence Export\n\n");
    markdown.push_str(&format!("**Export Date:** {}\n", Local::now().format("%Y-%m-%d %H:%M:%S")));
    markdown.push_str(&format!("**Account ID:** {}\n", request.account_id));
    markdown.push_str(&format!("**Total Emails:** {}\n\n", emails.len()));
    
    for (idx, email) in emails.iter().enumerate() {
        // Generate synopsis (first 3-5 lines of body)
        let synopsis = generate_synopsis(&email.body, 5);
        
        // Save email as .eml file
        let email_filename = format!("{:04}_{}.eml", idx + 1, sanitize_filename(&email.subject));
        let email_path = emails_dir.join(&email_filename);
        fs::write(&email_path, &email.raw_content)?;
        
        // Copy attachments
        let mut attachment_paths = Vec::new();
        let attachments = get_email_attachments(email.id).await?;
        
        for (att_idx, attachment) in attachments.iter().enumerate() {
            let source_path = PathBuf::from(&attachment.storage_path);
            if source_path.exists() {
                let att_filename = format!("{:04}_{:02}_{}", 
                    idx + 1, att_idx + 1, 
                    sanitize_filename(&attachment.filename)
                );
                let dest_path = attachments_dir.join(&att_filename);
                fs::copy(&source_path, &dest_path)?;
                attachment_paths.push(att_filename);
            }
        }
        
        // Write to CSV
        csv_writer.write_record(&[
            email.date.format("%Y-%m-%d %H:%M:%S").to_string(),
            email.from_addr.clone(),
            email.to_addr.clone().unwrap_or_default(),
            email.cc_addr.clone().unwrap_or_default(),
            email.subject.clone(),
            synopsis.clone(),
            attachment_paths.join("; "),
        ])?;
        
        // Add to markdown
        markdown.push_str(&format!("## Email {} - {}\n", idx + 1, email.subject));
        markdown.push_str(&format!("**Date:** {}\n", email.date.format("%Y-%m-%d %H:%M:%S")));
        markdown.push_str(&format!("**From:** {}\n", email.from_addr));
        markdown.push_str(&format!("**To:** {}\n", email.to_addr.as_ref().unwrap_or(&"N/A".to_string())));
        if !attachment_paths.is_empty() {
            markdown.push_str(&format!("**Attachments:** {}\n", attachment_paths.join(", ")));
        }
        markdown.push_str(&format!("**Synopsis:** {}\n\n", synopsis));
    }
    
    csv_writer.flush()?;
    
    // Write markdown summary
    let summary_path = export_dir.join("summary.md");
    fs::write(&summary_path, markdown)?;
    
    // Create README
    let readme = format!(
        "# Evidence Export README\n\n\
        This export contains {} emails with attachments.\n\n\
        ## Directory Structure\n\
        - `emails/` - Original email files in .eml format\n\
        - `attachments/` - All email attachments with numbered prefixes\n\
        - `manifest.csv` - Spreadsheet with email metadata and synopsis\n\
        - `summary.md` - Human-readable summary of all emails\n\n\
        ## File Naming Convention\n\
        - Emails: `NNNN_subject.eml` where NNNN is the sequence number\n\
        - Attachments: `NNNN_AA_filename` where AA is the attachment number\n",
        emails.len()
    );
    fs::write(export_dir.join("README.md"), readme)?;
    
    Ok(export_dir.to_string_lossy().to_string())
}

fn generate_synopsis(body: &str, max_lines: usize) -> String {
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .take(max_lines)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(500)
        .collect()
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(50)
        .collect()
}
```

**Test Strategy:**

1. Unit test synopsis generation with various email body formats
2. Test filename sanitization with special characters
3. Integration test with small email set to verify directory structure
4. Test CSV generation and verify all fields populated correctly
5. Test attachment copying with missing files gracefully handled

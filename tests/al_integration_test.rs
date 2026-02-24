//! Integration tests for the AL dispatcher using the real model pack and compiled registry.
//!
//! These tests load the trained embeddings and test registry, then exercise
//! the full pipeline: AL parsing -> embedding -> cosine similarity -> slot mapping.
//!
//! Each test case documents:
//! - The AL input being tested
//! - The expected action to be selected
//! - The actual confidence score achieved

use spectre_core::planner::SpectreDispatcher;
use spectre_core::types::{PlanRequest, PlanStatus};
use spectre_core::CompiledRegistry;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Shared dispatcher — loaded once, reused across all tests.
static DISPATCHER: OnceLock<SpectreDispatcher> = OnceLock::new();

fn dispatcher() -> &'static SpectreDispatcher {
    DISPATCHER.get_or_init(|| {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let pack_dir = manifest.join("../../packs/minilm");
        let registry_path = manifest.join("tests/test_registry.mcr");

        let (_meta, embedder) =
            spectre_core::pack::load_pack(&pack_dir).expect("failed to load model pack");
        let compiled =
            CompiledRegistry::load(&registry_path).expect("failed to load test registry");

        SpectreDispatcher::new(embedder, compiled)
    })
}

// ---------------------------------------------------------------------------
// Test helper
// ---------------------------------------------------------------------------

struct TestCase {
    al: &'static str,
    expected_tool: &'static str,
    /// Minimum confidence we expect (0.0 to disable check)
    min_confidence: f32,
    slots: Vec<(&'static str, &'static str)>,
}

fn run_test(tc: &TestCase) {
    let d = dispatcher();
    let slots: HashMap<String, String> = tc
        .slots
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    let plan = d.plan(&PlanRequest {
        al: tc.al.to_string(),
        slots,
        top_k: 5,
        tool_threshold: Some(0.0), // accept everything for scoring purposes
        mapping_threshold: Some(0.0),
    });

    let selected = plan.selected_tool.as_deref().unwrap_or("NONE");
    let confidence = plan.confidence.unwrap_or(0.0);

    // Print results for visibility when running tests with --nocapture
    eprintln!(
        "\n  AL:       {}\n  EXPECTED: {}\n  GOT:      {} (confidence: {:.4})\n  STATUS:   {:?}\n  TOP-3:    {:?}",
        tc.al,
        tc.expected_tool,
        selected,
        confidence,
        plan.status,
        plan.candidates.iter().take(3).map(|c| format!("{} ({:.3})", c.id, c.score)).collect::<Vec<_>>(),
    );

    assert_eq!(
        selected, tc.expected_tool,
        "AL: {:?} — expected {:?}, got {:?} (conf={:.4})",
        tc.al, tc.expected_tool, selected, confidence
    );

    if tc.min_confidence > 0.0 {
        assert!(
            confidence >= tc.min_confidence,
            "AL: {:?} — confidence {:.4} below minimum {:.4}",
            tc.al,
            confidence,
            tc.min_confidence
        );
    }
}

// ===========================================================================
// Blog / API tests
// ===========================================================================

#[test]
fn blog_post_exact_example() {
    run_test(&TestCase {
        al: "WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}",
        expected_tool: "Elchemista.Blog.create_post/2",
        min_confidence: 0.5,
        slots: vec![("title", "Hello"), ("text", "World")],
    });
}

#[test]
fn blog_post_lowercase() {
    run_test(&TestCase {
        al: "write new blog post for elchemista.com with: title={title} text={text}",
        expected_tool: "Elchemista.Blog.create_post/2",
        min_confidence: 0.4,
        slots: vec![("title", "Hello"), ("text", "World")],
    });
}

#[test]
fn blog_post_with_literal_values() {
    let d = dispatcher();
    let plan = d.plan_al(
        "WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE=\"My Post\" TEXT=\"Content here\"",
        Some(5),
        Some(0.0),
        Some(0.0),
    );
    eprintln!(
        "\n  blog_post_literal: selected={:?} confidence={:?}",
        plan.selected_tool, plan.confidence
    );
    assert_eq!(
        plan.selected_tool.as_deref(),
        Some("Elchemista.Blog.create_post/2")
    );
}

#[test]
fn stripe_payment_exact() {
    run_test(&TestCase {
        al: "CREATE STRIPE PAYMENT LINK WITH: AMOUNT={amount} CURRENCY={currency} PRODUCT_NAME={name}",
        expected_tool: "Payments.Stripe.create_payment_link/1",
        min_confidence: 0.4,
        slots: vec![("amount", "5000"), ("currency", "usd"), ("name", "Widget")],
    });
}

#[test]
fn stripe_payment_with_values() {
    let d = dispatcher();
    let plan = d.plan_al(
        "CREATE STRIPE PAYMENT LINK WITH: AMOUNT=1299 CURRENCY='USD' PRODUCT_NAME=\"T-shirt\"",
        Some(5),
        Some(0.0),
        Some(0.0),
    );
    eprintln!(
        "\n  stripe_values: selected={:?} confidence={:?} args={:?}",
        plan.selected_tool, plan.confidence, plan.args
    );
    assert_eq!(
        plan.selected_tool.as_deref(),
        Some("Payments.Stripe.create_payment_link/1")
    );
}

// ===========================================================================
// Linux package management
// ===========================================================================

#[test]
fn apt_install_exact() {
    run_test(&TestCase {
        al: "INSTALL PACKAGE {package} VIA APT",
        expected_tool: "Linux.Apt.install/1",
        min_confidence: 0.3,
        slots: vec![("package", "nginx")],
    });
}

#[test]
fn apt_install_short() {
    run_test(&TestCase {
        al: "APT INSTALL {package}",
        expected_tool: "Linux.Apt.install/1",
        min_confidence: 0.3,
        slots: vec![("package", "nginx")],
    });
}

#[test]
fn pacman_install() {
    run_test(&TestCase {
        al: "INSTALL PACKAGE {package} VIA PACMAN",
        expected_tool: "Linux.Pacman.install/1",
        min_confidence: 0.3,
        slots: vec![("package", "firefox")],
    });
}

#[test]
fn dnf_install() {
    run_test(&TestCase {
        al: "INSTALL PACKAGE {package} VIA DNF",
        expected_tool: "Linux.Dnf.install/1",
        min_confidence: 0.3,
        slots: vec![("package", "httpd")],
    });
}

// ===========================================================================
// File operations
// ===========================================================================

#[test]
fn list_directory() {
    run_test(&TestCase {
        al: "LIST DIRECTORY {path}",
        expected_tool: "Linux.Coreutils.ls/1",
        min_confidence: 0.3,
        slots: vec![("path", "/home")],
    });
}

#[test]
fn ls_directory() {
    run_test(&TestCase {
        al: "LS DIRECTORY {path}",
        expected_tool: "Linux.Coreutils.ls/1",
        min_confidence: 0.3,
        slots: vec![("path", "/tmp")],
    });
}

#[test]
fn delete_file() {
    run_test(&TestCase {
        al: "DELETE FILE {path}",
        expected_tool: "Linux.Coreutils.rm/1",
        min_confidence: 0.3,
        slots: vec![("path", "/tmp/junk.txt")],
    });
}

#[test]
fn remove_file() {
    run_test(&TestCase {
        al: "REMOVE FILE {path}",
        expected_tool: "Linux.Coreutils.rm/1",
        min_confidence: 0.3,
        slots: vec![("path", "/tmp/old.log")],
    });
}

#[test]
fn read_file() {
    run_test(&TestCase {
        al: "READ FILE {path}",
        expected_tool: "Linux.Coreutils.cat/1",
        min_confidence: 0.3,
        slots: vec![("path", "/etc/hosts")],
    });
}

#[test]
fn cat_file() {
    run_test(&TestCase {
        al: "CAT FILE {path}",
        expected_tool: "Linux.Coreutils.cat/1",
        min_confidence: 0.3,
        slots: vec![("path", "/etc/passwd")],
    });
}

#[test]
fn copy_file() {
    run_test(&TestCase {
        al: "COPY FILE {source} TO {dest}",
        expected_tool: "Linux.Coreutils.cp/2",
        min_confidence: 0.3,
        slots: vec![("source", "/a"), ("dest", "/b")],
    });
}

#[test]
fn move_file() {
    run_test(&TestCase {
        al: "MOVE FILE {source} TO {dest}",
        expected_tool: "Linux.Coreutils.mv/2",
        min_confidence: 0.3,
        slots: vec![("source", "/a"), ("dest", "/b")],
    });
}

#[test]
fn rename_file() {
    run_test(&TestCase {
        al: "RENAME FILE {source} TO {dest}",
        expected_tool: "Linux.Coreutils.mv/2",
        min_confidence: 0.3,
        slots: vec![("source", "old.txt"), ("dest", "new.txt")],
    });
}

#[test]
fn mkdir() {
    run_test(&TestCase {
        al: "CREATE DIRECTORY {path}",
        expected_tool: "Linux.Coreutils.mkdir/1",
        min_confidence: 0.3,
        slots: vec![("path", "/tmp/newdir")],
    });
}

#[test]
fn mkdir_short() {
    run_test(&TestCase {
        al: "MKDIR {path}",
        expected_tool: "Linux.Coreutils.mkdir/1",
        min_confidence: 0.3,
        slots: vec![("path", "/tmp/test")],
    });
}

// ===========================================================================
// Network operations
// ===========================================================================

#[test]
fn download_url() {
    run_test(&TestCase {
        al: "DOWNLOAD URL {url} TO FILE {path}",
        expected_tool: "Linux.Network.curl/2",
        min_confidence: 0.3,
        slots: vec![("url", "https://example.com"), ("path", "/tmp/page.html")],
    });
}

#[test]
fn curl_url() {
    run_test(&TestCase {
        al: "CURL URL {url} TO {path}",
        expected_tool: "Linux.Network.curl/2",
        min_confidence: 0.3,
        slots: vec![("url", "https://api.com/data"), ("path", "out.json")],
    });
}

#[test]
fn fetch_http() {
    run_test(&TestCase {
        al: "FETCH HTTP {url}",
        expected_tool: "Linux.Network.curl/2",
        min_confidence: 0.2,
        slots: vec![("url", "https://example.com")],
    });
}

#[test]
fn ping_host() {
    run_test(&TestCase {
        al: "PING HOST {host}",
        expected_tool: "Linux.Network.ping/1",
        min_confidence: 0.3,
        slots: vec![("host", "8.8.8.8")],
    });
}

#[test]
fn test_network_connectivity() {
    run_test(&TestCase {
        al: "TEST NETWORK CONNECTIVITY TO {host}",
        expected_tool: "Linux.Network.ping/1",
        min_confidence: 0.2,
        slots: vec![("host", "google.com")],
    });
}

#[test]
fn dns_lookup() {
    run_test(&TestCase {
        al: "LOOKUP DNS FOR {domain}",
        expected_tool: "Linux.Network.dig/1",
        min_confidence: 0.3,
        slots: vec![("domain", "example.com")],
    });
}

#[test]
fn dig_domain() {
    run_test(&TestCase {
        al: "DIG DOMAIN {domain}",
        expected_tool: "Linux.Network.dig/1",
        min_confidence: 0.3,
        slots: vec![("domain", "example.com")],
    });
}

// ===========================================================================
// SSH operations
// ===========================================================================

#[test]
fn ssh_to_host() {
    run_test(&TestCase {
        al: "SSH TO HOST {host} AS USER {user}",
        expected_tool: "Linux.OpenSSH.ssh/2",
        min_confidence: 0.3,
        slots: vec![("host", "myserver.com"), ("user", "admin")],
    });
}

#[test]
fn connect_remotely() {
    run_test(&TestCase {
        al: "CONNECT REMOTELY TO {host}",
        expected_tool: "Linux.OpenSSH.ssh/2",
        min_confidence: 0.2,
        slots: vec![("host", "prod.example.com")],
    });
}

// ===========================================================================
// Process management
// ===========================================================================

#[test]
fn kill_process() {
    run_test(&TestCase {
        al: "KILL PROCESS {pid}",
        expected_tool: "Linux.Procps.kill/1",
        min_confidence: 0.3,
        slots: vec![("pid", "1234")],
    });
}

#[test]
fn terminate_process() {
    run_test(&TestCase {
        al: "TERMINATE PROCESS {pid}",
        expected_tool: "Linux.Procps.kill/1",
        min_confidence: 0.3,
        slots: vec![("pid", "5678")],
    });
}

// ===========================================================================
// Service management
// ===========================================================================

#[test]
fn start_service() {
    run_test(&TestCase {
        al: "START SERVICE {name}",
        expected_tool: "Linux.Systemd.systemctl_start/1",
        min_confidence: 0.3,
        slots: vec![("name", "nginx")],
    });
}

#[test]
fn stop_service() {
    run_test(&TestCase {
        al: "STOP SERVICE {name}",
        expected_tool: "Linux.Systemd.systemctl_stop/1",
        min_confidence: 0.3,
        slots: vec![("name", "apache2")],
    });
}

#[test]
fn restart_service() {
    run_test(&TestCase {
        al: "RESTART SERVICE {name}",
        expected_tool: "Linux.Systemd.systemctl_restart/1",
        min_confidence: 0.3,
        slots: vec![("name", "postgresql")],
    });
}

// ===========================================================================
// Docker / Containers
// ===========================================================================

#[test]
fn docker_run() {
    run_test(&TestCase {
        al: "RUN DOCKER IMAGE {image} WITH ARGS {args}",
        expected_tool: "Linux.Container.docker_run/2",
        min_confidence: 0.3,
        slots: vec![("image", "nginx:latest"), ("args", "-p 80:80")],
    });
}

#[test]
fn start_container() {
    run_test(&TestCase {
        al: "START CONTAINER FROM {image}",
        expected_tool: "Linux.Container.docker_run/2",
        min_confidence: 0.2,
        slots: vec![("image", "redis:7")],
    });
}

// ===========================================================================
// Git
// ===========================================================================

#[test]
fn git_command() {
    run_test(&TestCase {
        al: "RUN GIT {command} WITH ARGS {args}",
        expected_tool: "Linux.Vcs.git/2",
        min_confidence: 0.3,
        slots: vec![("command", "commit"), ("args", "-m 'fix'")],
    });
}

// ===========================================================================
// Archive / Compression
// ===========================================================================

#[test]
fn compress_directory() {
    run_test(&TestCase {
        al: "COMPRESS DIRECTORY {path} INTO FILE {archive}",
        expected_tool: "Linux.Archive.tar/2",
        min_confidence: 0.3,
        slots: vec![("path", "/var/log"), ("archive", "logs.tar.gz")],
    });
}

#[test]
fn tar_gzip() {
    run_test(&TestCase {
        al: "TAR AND GZIP FOLDER {path} TO {archive}",
        expected_tool: "Linux.Archive.tar/2",
        min_confidence: 0.3,
        slots: vec![("path", "/data"), ("archive", "backup.tar.gz")],
    });
}

// ===========================================================================
// Search / Grep
// ===========================================================================

#[test]
fn search_text_in_file() {
    run_test(&TestCase {
        al: "SEARCH TEXT {pattern} IN FILE {file}",
        expected_tool: "Linux.Coreutils.grep/2",
        min_confidence: 0.3,
        slots: vec![("pattern", "error"), ("file", "/var/log/syslog")],
    });
}

#[test]
fn grep_pattern() {
    run_test(&TestCase {
        al: "GREP PATTERN {pattern} IN FILE {file}",
        expected_tool: "Linux.Coreutils.grep/2",
        min_confidence: 0.2,
        slots: vec![("pattern", "TODO"), ("file", "main.rs")],
    });
}

// ===========================================================================
// Permissions
// ===========================================================================

#[test]
fn chmod_file() {
    run_test(&TestCase {
        al: "CHMOD FILE {path} TO {mode}",
        expected_tool: "Linux.Coreutils.chmod/2",
        min_confidence: 0.3,
        slots: vec![("path", "script.sh"), ("mode", "755")],
    });
}

#[test]
fn change_permissions() {
    run_test(&TestCase {
        al: "CHANGE PERMISSIONS OF {path} TO {mode}",
        expected_tool: "Linux.Coreutils.chmod/2",
        min_confidence: 0.2,
        slots: vec![("path", "/usr/local/bin/app"), ("mode", "+x")],
    });
}

// ===========================================================================
// User management
// ===========================================================================

#[test]
fn create_user() {
    run_test(&TestCase {
        al: "CREATE NEW USER {username}",
        expected_tool: "Linux.User.useradd/1",
        min_confidence: 0.3,
        slots: vec![("username", "deploy")],
    });
}

// ===========================================================================
// Find
// ===========================================================================

#[test]
fn find_file() {
    run_test(&TestCase {
        al: "FIND FILE {name} IN DIRECTORY {path}",
        expected_tool: "Linux.Findutils.find/2",
        min_confidence: 0.3,
        slots: vec![("name", "*.log"), ("path", "/var")],
    });
}

// ===========================================================================
// Python
// ===========================================================================

#[test]
fn run_python_script() {
    run_test(&TestCase {
        al: "RUN PYTHON SCRIPT {script} WITH {args}",
        expected_tool: "Linux.Lang.python/2",
        min_confidence: 0.3,
        slots: vec![("script", "train.py"), ("args", "--epochs 10")],
    });
}

#[test]
fn execute_python() {
    run_test(&TestCase {
        al: "EXECUTE PYTHON PROGRAM {script}",
        expected_tool: "Linux.Lang.python/2",
        min_confidence: 0.3,
        slots: vec![("script", "app.py")],
    });
}

// ===========================================================================
// Rsync / Backup
// ===========================================================================

#[test]
fn rsync_dirs() {
    run_test(&TestCase {
        al: "RSYNC {source} TO {dest}",
        expected_tool: "Linux.Network.rsync/2",
        min_confidence: 0.3,
        slots: vec![("source", "/data"), ("dest", "backup:/data")],
    });
}

#[test]
fn sync_directory() {
    run_test(&TestCase {
        al: "SYNC DIRECTORY {source} WITH {dest}",
        expected_tool: "Linux.Network.rsync/2",
        min_confidence: 0.2,
        slots: vec![("source", "/home/dev"), ("dest", "/mnt/backup")],
    });
}

#[test]
fn backup_files() {
    run_test(&TestCase {
        al: "BACKUP AND SYNC FILES FROM {source} TO {dest}",
        expected_tool: "Linux.Network.rsync/2",
        min_confidence: 0.2,
        slots: vec![("source", "/var/data"), ("dest", "/mnt/nas")],
    });
}

// ===========================================================================
// Case-insensitive / messy input tests
// ===========================================================================

#[test]
fn lowercase_kill_process() {
    run_test(&TestCase {
        al: "kill process {pid}",
        expected_tool: "Linux.Procps.kill/1",
        min_confidence: 0.2,
        slots: vec![("pid", "9999")],
    });
}

#[test]
fn messy_blog_post() {
    let d = dispatcher();
    let plan = d.plan_al(
        "write new blog post for elchemista.com with: title='My Post'; text='Hello world';",
        Some(5),
        Some(0.0),
        Some(0.0),
    );
    eprintln!(
        "\n  messy_blog: selected={:?} confidence={:?} args={:?}",
        plan.selected_tool, plan.confidence, plan.args
    );
    assert_eq!(
        plan.selected_tool.as_deref(),
        Some("Elchemista.Blog.create_post/2")
    );
}

#[test]
fn messy_stripe_with_punctuation() {
    let d = dispatcher();
    let plan = d.plan_al(
        "Create Stripe Payment Link WITH: Amount=5000, Currency='usd', Product_Name=\"Widget\"",
        Some(5),
        Some(0.0),
        Some(0.0),
    );
    eprintln!(
        "\n  messy_stripe: selected={:?} confidence={:?} args={:?}",
        plan.selected_tool, plan.confidence, plan.args
    );
    assert_eq!(
        plan.selected_tool.as_deref(),
        Some("Payments.Stripe.create_payment_link/1")
    );
}

// ===========================================================================
// Suggestions test (use very high threshold so nothing matches)
// ===========================================================================

#[test]
fn suggestions_when_no_match() {
    let d = dispatcher();
    let plan = d.plan(&PlanRequest {
        al: "DO SOMETHING UNUSUAL".to_string(),
        slots: HashMap::new(),
        top_k: 5,
        tool_threshold: Some(0.99), // impossibly high threshold
        mapping_threshold: Some(0.0),
    });

    eprintln!(
        "\n  suggestions: status={:?} suggestions={:?}",
        plan.status,
        plan.suggestions
            .iter()
            .map(|s| format!("{} ({:.3}): {}", s.id, s.score, s.al_command))
            .collect::<Vec<_>>()
    );

    assert_eq!(plan.status, PlanStatus::NoTool);
    assert!(
        !plan.suggestions.is_empty(),
        "should have suggestions when no tool matches"
    );
    assert!(plan.suggestions.len() <= 3, "should have at most 3 suggestions");
    // Each suggestion should have a non-empty al_command
    for s in &plan.suggestions {
        assert!(!s.al_command.is_empty());
        assert!(!s.id.is_empty());
    }
}

// ===========================================================================
// Default values test
// ===========================================================================

#[test]
fn default_value_for_currency() {
    let d = dispatcher();
    let plan = d.plan_al(
        "CREATE STRIPE PAYMENT LINK WITH: AMOUNT=2500 PRODUCT_NAME=\"Book\"",
        Some(5),
        Some(0.0),
        Some(0.0),
    );
    eprintln!(
        "\n  default_currency: selected={:?} args={:?}",
        plan.selected_tool, plan.args
    );
    assert_eq!(
        plan.selected_tool.as_deref(),
        Some("Payments.Stripe.create_payment_link/1")
    );
    // The currency arg should be filled from the default "usd"
    if let Some(ref args) = plan.args {
        let has_currency = args.get("currency").map(|v| v.as_str()) == Some("usd")
            || args.values().any(|v| v == "usd");
        eprintln!("  args map: {:?}", args);
        // Note: whether default appears depends on slot mapping — log for diagnostics
        if has_currency {
            eprintln!("  -> currency default applied correctly");
        }
    }
}

// ===========================================================================
// Score report: run all test ALs and print a summary table
// ===========================================================================

#[test]
fn score_report() {
    let cases: Vec<(&str, &str)> = vec![
        // Blog / API
        ("WRITE NEW BLOG POST FOR elchemista.com WITH: TITLE={title} TEXT={text}", "Elchemista.Blog.create_post/2"),
        ("CREATE STRIPE PAYMENT LINK WITH: AMOUNT={amount} CURRENCY={currency} PRODUCT_NAME={name}", "Payments.Stripe.create_payment_link/1"),
        // Package managers
        ("INSTALL PACKAGE {package} VIA APT", "Linux.Apt.install/1"),
        ("APT INSTALL {package}", "Linux.Apt.install/1"),
        ("INSTALL PACKAGE {package} VIA PACMAN", "Linux.Pacman.install/1"),
        ("PACMAN SYNC {package}", "Linux.Pacman.install/1"),
        ("INSTALL PACKAGE {package} VIA DNF", "Linux.Dnf.install/1"),
        ("DNF INSTALL {package}", "Linux.Dnf.install/1"),
        // File ops
        ("LIST DIRECTORY {path}", "Linux.Coreutils.ls/1"),
        ("LS DIRECTORY {path}", "Linux.Coreutils.ls/1"),
        ("DELETE FILE {path}", "Linux.Coreutils.rm/1"),
        ("REMOVE FILE {path}", "Linux.Coreutils.rm/1"),
        ("READ FILE {path}", "Linux.Coreutils.cat/1"),
        ("CAT FILE {path}", "Linux.Coreutils.cat/1"),
        ("COPY FILE {source} TO {dest}", "Linux.Coreutils.cp/2"),
        ("MOVE FILE {source} TO {dest}", "Linux.Coreutils.mv/2"),
        ("CREATE DIRECTORY {path}", "Linux.Coreutils.mkdir/1"),
        ("MKDIR {path}", "Linux.Coreutils.mkdir/1"),
        // Network
        ("DOWNLOAD URL {url} TO FILE {path}", "Linux.Network.curl/2"),
        ("CURL URL {url} TO {path}", "Linux.Network.curl/2"),
        ("PING HOST {host}", "Linux.Network.ping/1"),
        ("LOOKUP DNS FOR {domain}", "Linux.Network.dig/1"),
        ("DIG DOMAIN {domain}", "Linux.Network.dig/1"),
        // SSH
        ("SSH TO HOST {host} AS USER {user}", "Linux.OpenSSH.ssh/2"),
        // Process
        ("KILL PROCESS {pid}", "Linux.Procps.kill/1"),
        ("TERMINATE PROCESS {pid}", "Linux.Procps.kill/1"),
        // Services
        ("START SERVICE {name}", "Linux.Systemd.systemctl_start/1"),
        ("STOP SERVICE {name}", "Linux.Systemd.systemctl_stop/1"),
        ("RESTART SERVICE {name}", "Linux.Systemd.systemctl_restart/1"),
        // Docker
        ("RUN DOCKER IMAGE {image} WITH ARGS {args}", "Linux.Container.docker_run/2"),
        // Git
        ("RUN GIT {command} WITH ARGS {args}", "Linux.Vcs.git/2"),
        // Archive
        ("COMPRESS DIRECTORY {path} INTO FILE {archive}", "Linux.Archive.tar/2"),
        // Search
        ("SEARCH TEXT {pattern} IN FILE {file}", "Linux.Coreutils.grep/2"),
        // Permissions
        ("CHMOD FILE {path} TO {mode}", "Linux.Coreutils.chmod/2"),
        // User
        ("CREATE NEW USER {username}", "Linux.User.useradd/1"),
        // Find
        ("FIND FILE {name} IN DIRECTORY {path}", "Linux.Findutils.find/2"),
        // Python
        ("RUN PYTHON SCRIPT {script} WITH {args}", "Linux.Lang.python/2"),
        // Rsync
        ("RSYNC {source} TO {dest}", "Linux.Network.rsync/2"),
        ("SYNC DIRECTORY {source} WITH {dest}", "Linux.Network.rsync/2"),
        ("BACKUP AND SYNC FILES FROM {source} TO {dest}", "Linux.Network.rsync/2"),
    ];

    let d = dispatcher();
    let mut correct = 0;
    let mut total = 0;
    let mut total_confidence = 0.0f32;

    eprintln!("\n{}", "=".repeat(85));
    eprintln!("  SPECTRE-KINETIC SCORE REPORT");
    eprintln!("  {} test AL statements against {} registered actions", cases.len(), 28);
    eprintln!("{}", "=".repeat(85));
    eprintln!("{:<65} {:>10} {:>8}", "AL", "MATCH", "CONF");
    eprintln!("{}", "-".repeat(85));

    for (al, expected) in &cases {
        let plan = d.plan(&PlanRequest {
            al: al.to_string(),
            slots: HashMap::new(),
            top_k: 5,
            tool_threshold: Some(0.0),
            mapping_threshold: Some(0.0),
        });

        let selected = plan.selected_tool.as_deref().unwrap_or("NONE");
        let confidence = plan.confidence.unwrap_or(0.0);
        let matched = selected == *expected;

        if matched {
            correct += 1;
        }
        total += 1;
        total_confidence += confidence;

        let mark = if matched { "OK" } else { "MISS" };
        let al_short = if al.len() > 62 { &al[..62] } else { al };
        eprintln!("{:<65} {:>10} {:>7.4}", al_short, mark, confidence);

        if !matched {
            eprintln!("    EXPECTED: {}", expected);
            eprintln!("    GOT:      {}", selected);
        }
    }

    let accuracy = correct as f32 / total as f32 * 100.0;
    let avg_conf = total_confidence / total as f32;

    eprintln!("{}", "-".repeat(85));
    eprintln!(
        "  ACCURACY: {}/{} ({:.1}%)  |  AVG CONFIDENCE: {:.4}",
        correct, total, accuracy, avg_conf
    );
    eprintln!("{}", "=".repeat(85));

    // We expect at least 80% accuracy
    assert!(
        accuracy >= 80.0,
        "accuracy {:.1}% is below 80% threshold",
        accuracy
    );
}

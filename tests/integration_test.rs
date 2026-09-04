/// 集成测试示例
use audiofn::cli::{self, Cli};
use clap::{CommandFactory, Parser};

#[test]
fn test_cli_config_output() {
    // 验证 CLI 可以正确解析 config 命令
    let cli = Cli::try_parse_from(["test", "config"]).unwrap();
    assert!(matches!(cli.command.unwrap(), cli::Commands::Config));
}

#[tokio::test]
async fn test_run_config_returns_ok() {
    let cli = Cli::try_parse_from(["test", "config"]).unwrap();
    let result = cli::run(cli).await;
    assert!(result.is_ok());
}

#[test]
fn test_cli_help_has_no_legacy_brand_or_greet() {
    // CLI 面验收：帮助文案不含旧品牌与已删除的 greet
    let help = Cli::command().render_help().to_string();
    assert!(!help.to_lowercase().contains("zapmomo"), "help: {help}");
    assert!(!help.contains("greet"), "help: {help}");
}

#[test]
fn test_datetime_iso_format() {
    let now = audiofn::datetime::iso_timestamp_now();
    assert!(
        now.contains('T'),
        "ISO 8601 timestamp should contain T separator"
    );
}

#[test]
fn test_logging_init() {
    // 初始化日志不应 panic
    audiofn::logging::init_logging();
}

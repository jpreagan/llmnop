use anyhow::Result;
use axoupdater::AxoUpdater;

pub async fn run_update() -> Result<()> {
    let mut updater = AxoUpdater::new_for("llmnop");

    if updater.load_receipt().is_err() {
        eprintln!(
            "Self-update is only available for standalone installs. Use your package manager to upgrade."
        );
        return Ok(());
    }

    if !updater.check_receipt_is_for_this_executable()? {
        eprintln!(
            "Self-update is only available for standalone installs. Use your package manager to upgrade."
        );
        return Ok(());
    }

    match updater.run().await? {
        Some(result) => {
            let old_version = result
                .old_version
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "unknown".to_string());
            let new_version = result.new_version.to_string();
            println!("Updated llmnop from {} to {}.", old_version, new_version);
        }
        None => {
            println!("llmnop is already up to date.");
        }
    }

    Ok(())
}

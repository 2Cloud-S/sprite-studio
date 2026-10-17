use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 1 || args.len() > 2 || (args.len() == 2 && args[1] != "--apply") {
        eprintln!("Usage: reconcile_workspace <absolute-project.json> [--apply]\nWithout --apply this is a read-only dry run.");
        std::process::exit(2);
    }
    match sprite_studio_lib::recovery::reconcile_project_file(Path::new(&args[0]), args.len() == 2)
    {
        Ok(report) => {
            for entry in &report.entries {
                println!("{}: {}", entry.animation, entry.result);
            }
            println!(
                "Recovered complete: {} | recovered review: {} | still stale: {} | missing/corrupt: {} | unchanged: {}",
                report.recovered_complete,
                report.recovered_review,
                report.still_stale,
                report.missing_or_corrupt,
                report.unchanged
            );
            if let Some(path) = report.backup_path {
                println!("Pre-reconcile metadata backup: {}", path.display());
            }
            if let Err(error) = sprite_studio_lib::verify_project_file(Path::new(&args[0])) {
                eprintln!("Typed Sprite Studio project validation failed: {error}");
                std::process::exit(1);
            }
            println!("Typed Sprite Studio project validation: passed");
        }
        Err(error) => {
            eprintln!("Reconciliation stopped: {error}");
            std::process::exit(1);
        }
    }
}

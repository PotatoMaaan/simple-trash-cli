use anyhow::Context;
use chrono::NaiveTime;

pub fn empty(args: crate::cli::EmptyArgs, trash: crate::UnifiedTrash) -> anyhow::Result<()> {
    let older_than = args
        .before_datetime
        .or(args
            .before_date
            .map(|x| x.and_time(NaiveTime::from_num_seconds_from_midnight_opt(0, 0).unwrap())))
        .unwrap_or(chrono::Local::now().naive_local());

    trash
        .empty(older_than, args.dry_run)
        .context("Failed to empty trash")?;

    println!("Emptied trash!");
    Ok(())
}
